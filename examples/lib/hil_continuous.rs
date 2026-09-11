use crate::hil::HilOptions;
use e710_uhf::connector::{AsyncIO, Connector};
use e710_uhf::frame::command::{Command, CommandResult, ReadResult};
use e710_uhf::tag::Tag;
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;

const CSV_HEADER: &str = "cycle_id,cycle_start_unix_ns,cycle_start_elapsed_us,repeat_period_us,idle_gap_us,inventory_io_us,buffer_wait_us,cycle_total_us,status,error,tag_count,tag_index,tag_received_unix_ns,tag_received_utc,antenna_index,antenna_port,frequency_mhz,rssi,pc,epc,phase_i,phase_q,reader_total_read,reader_read_rate\n";

pub struct ContinuousInventorySummary {
    pub cycles: u64,
    pub failed_cycles: u64,
    pub total_tags: u64,
    pub output: PathBuf,
}

struct CycleRecord {
    cycle_id: u64,
    cycle_start_unix_ns: u128,
    cycle_start_elapsed: Duration,
    repeat_period: Duration,
    idle_gap: Duration,
    inventory_io: Duration,
    buffer_wait: Duration,
    cycle_total: Duration,
    tags: Vec<Tag>,
    reader_result: Option<ReadResult>,
    error: Option<String>,
}

struct WriterSummary {
    cycles: u64,
    rows: u64,
}

pub async fn run<S>(
    connector: &mut Connector<S>,
    command: Command,
    options: &HilOptions,
) -> Result<ContinuousInventorySummary, String>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    if options.inventory_duration.is_zero() {
        return Err("continuous inventory duration must be greater than zero".to_owned());
    }
    if options.buffer_capacity == 0 {
        return Err("inventory buffer capacity must be greater than zero".to_owned());
    }
    let benchmark_started = Instant::now();
    let deadline = benchmark_started
        .checked_add(options.inventory_duration)
        .ok_or_else(|| "continuous inventory duration is too large".to_owned())?;

    let output = output_path(options);
    let file = open_output(&output)?;
    let (sender, receiver) = mpsc::channel(options.buffer_capacity);
    let writer_output = output.clone();
    let writer = thread::Builder::new()
        .name("e710-hil-csv-writer".to_owned())
        .spawn(move || write_records(file, receiver))
        .map_err(|error| format!("cannot start CSV writer thread: {error}"))?;

    println!(
        "[INFO] continuous inventory: duration {:.3} s, buffer {} cycle(s), output {}",
        options.inventory_duration.as_secs_f64(),
        options.buffer_capacity,
        output.display()
    );

    let mut previous_start = None;
    let mut previous_completed = None;
    let mut cycles = 0_u64;
    let mut failed_cycles = 0_u64;
    let mut total_tags = 0_u64;
    let mut channel_error = None;

    while Instant::now() < deadline {
        cycles += 1;
        let cycle_started = Instant::now();
        let cycle_start_unix_ns = unix_time_ns();
        let repeat_period = previous_start
            .map(|previous: Instant| cycle_started.duration_since(previous))
            .unwrap_or_default();
        let idle_gap = previous_completed
            .map(|previous: Instant| cycle_started.duration_since(previous))
            .unwrap_or_default();

        let inventory_started = Instant::now();
        let response = connector.send_and_read_command(command.clone()).await;
        let inventory_io = inventory_started.elapsed();

        let (tags, reader_result, error) = match response {
            Ok(CommandResult::ResponsePackets(Ok((tags, result)))) => {
                total_tags += tags.len() as u64;
                (tags, Some(result), None)
            }
            Ok(CommandResult::ResponsePackets(Err(error))) => {
                failed_cycles += 1;
                (Vec::new(), None, Some(error.to_string()))
            }
            Ok(other) => {
                failed_cycles += 1;
                (
                    Vec::new(),
                    None,
                    Some(format!("unexpected response variant: {other:?}")),
                )
            }
            Err(error) => {
                failed_cycles += 1;
                (Vec::new(), None, Some(error.to_string()))
            }
        };
        let tag_count = tags.len();

        // Waiting for a permit measures backpressure without blocking a Tokio
        // worker. The permit guarantees that the following send cannot wait.
        let buffer_started = Instant::now();
        let permit = match sender.reserve().await {
            Ok(permit) => permit,
            Err(_) => {
                channel_error = Some(format!("CSV writer stopped before cycle {cycles}"));
                break;
            }
        };
        let buffer_wait = buffer_started.elapsed();
        let cycle_total = cycle_started.elapsed();

        let status = if error.is_some() { "ERROR" } else { "OK" };
        println!(
            "[CYCLE {cycles:06}] status={status} tags={tag_count} read={} us buffer={} us repeat={} us idle={} us total={} us",
            micros(inventory_io),
            micros(buffer_wait),
            micros(repeat_period),
            micros(idle_gap),
            micros(cycle_total),
        );

        permit.send(CycleRecord {
            cycle_id: cycles,
            cycle_start_unix_ns,
            cycle_start_elapsed: cycle_started.duration_since(benchmark_started),
            repeat_period,
            idle_gap,
            inventory_io,
            buffer_wait,
            cycle_total,
            tags,
            reader_result,
            error,
        });

        previous_start = Some(cycle_started);
        previous_completed = Some(Instant::now());
    }

    drop(sender);
    let writer_result = writer
        .join()
        .map_err(|_| "CSV writer thread panicked".to_owned())?;
    if let Some(channel_error) = channel_error {
        return Err(match writer_result {
            Ok(_) => channel_error,
            Err(writer_error) => format!("{channel_error}: {writer_error}"),
        });
    }
    let writer_summary = writer_result?;
    if writer_summary.cycles != cycles {
        return Err(format!(
            "CSV writer persisted {} of {cycles} cycles to {}",
            writer_summary.cycles,
            writer_output.display()
        ));
    }

    println!(
        "[INFO] writer completed: {} cycles and {} CSV rows persisted",
        writer_summary.cycles, writer_summary.rows
    );

    Ok(ContinuousInventorySummary {
        cycles,
        failed_cycles,
        total_tags,
        output,
    })
}

fn output_path(options: &HilOptions) -> PathBuf {
    options.inventory_output.clone().unwrap_or_else(|| {
        PathBuf::from(format!(
            "async_hil_inventory_{}.csv",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        ))
    })
}

fn open_output(path: &PathBuf) -> Result<File, String> {
    OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .map_err(|error| format!("cannot create inventory output {}: {error}", path.display()))
}

fn write_records(
    file: File,
    mut receiver: mpsc::Receiver<CycleRecord>,
) -> Result<WriterSummary, String> {
    let mut writer = BufWriter::new(file);
    writer
        .write_all(CSV_HEADER.as_bytes())
        .map_err(|error| format!("cannot write CSV header: {error}"))?;
    let mut cycles = 0_u64;
    let mut rows = 0_u64;

    while let Some(record) = receiver.blocking_recv() {
        rows += write_record(&mut writer, &record)?;
        cycles += 1;
        writer
            .flush()
            .map_err(|error| format!("cannot flush cycle {}: {error}", record.cycle_id))?;
    }

    Ok(WriterSummary { cycles, rows })
}

fn write_record(writer: &mut impl Write, record: &CycleRecord) -> Result<u64, String> {
    if record.tags.is_empty() {
        write_row(writer, record, None, None)?;
        return Ok(1);
    }

    for (tag_index, tag) in record.tags.iter().enumerate() {
        write_row(writer, record, Some(tag_index), Some(tag))?;
    }
    Ok(record.tags.len() as u64)
}

fn write_row(
    writer: &mut impl Write,
    record: &CycleRecord,
    tag_index: Option<usize>,
    tag: Option<&Tag>,
) -> Result<(), String> {
    let status = if record.error.is_some() {
        "ERROR"
    } else {
        "OK"
    };
    let error = csv_field(record.error.as_deref().unwrap_or(""));
    let (reader_total, reader_rate) = record
        .reader_result
        .as_ref()
        .map(|result| (result.total_read.to_string(), result.read_rate.to_string()))
        .unwrap_or_default();

    let mut columns = vec![
        record.cycle_id.to_string(),
        record.cycle_start_unix_ns.to_string(),
        micros(record.cycle_start_elapsed).to_string(),
        micros(record.repeat_period).to_string(),
        micros(record.idle_gap).to_string(),
        micros(record.inventory_io).to_string(),
        micros(record.buffer_wait).to_string(),
        micros(record.cycle_total).to_string(),
        status.to_owned(),
        error,
        record.tags.len().to_string(),
    ];

    if let Some(tag) = tag {
        columns.extend([
            tag_index.unwrap_or_default().to_string(),
            tag.received_at_ns.to_string(),
            csv_field(&tag.received_at_utc.to_rfc3339()),
            tag.antenna_index()
                .map_or_else(String::new, |v| v.to_string()),
            tag.antenna_id.to_string(),
            format!("{:.3}", tag.frequency),
            tag.rssi.to_string(),
            csv_field(&tag.pc),
            csv_field(&tag.epc),
            tag.phase.0.to_string(),
            tag.phase.1.to_string(),
        ]);
    } else {
        columns.extend(std::iter::repeat_n(String::new(), 11));
    }
    columns.extend([reader_total, reader_rate]);

    writeln!(writer, "{}", columns.join(","))
        .map_err(|error| format!("cannot write cycle {}: {error}", record.cycle_id))
}

fn csv_field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

fn unix_time_ns() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

fn micros(duration: Duration) -> u128 {
    duration.as_micros()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn csv_record_writes_one_complete_row_per_tag() {
        let record = CycleRecord {
            cycle_id: 3,
            cycle_start_unix_ns: 123,
            cycle_start_elapsed: Duration::from_micros(10),
            repeat_period: Duration::from_micros(20),
            idle_gap: Duration::from_micros(2),
            inventory_io: Duration::from_micros(15),
            buffer_wait: Duration::from_micros(1),
            cycle_total: Duration::from_micros(16),
            tags: vec![Tag {
                frequency: 866.0,
                antenna_id: 3,
                epc: "E28069150000501D63E29C4F".to_owned(),
                pc: "3000".to_owned(),
                rssi: 72,
                phase: (1, 2),
                received_at_ns: 456,
                received_at_utc: Utc::now(),
                antenna_choosing: Some(1),
            }],
            reader_result: Some(ReadResult {
                antenna_id: 0,
                read_rate: 18,
                total_read: 1,
            }),
            error: None,
        };
        let mut output = Vec::new();

        assert_eq!(write_record(&mut output, &record).unwrap(), 1);

        let row = String::from_utf8(output).unwrap();
        assert_eq!(row.trim_end().split(',').count(), 24);
        assert!(row.contains("E28069150000501D63E29C4F"));
        assert!(row.contains(",7,3,866.000,72,"));
    }

    #[test]
    fn csv_record_preserves_cycles_without_tags() {
        let record = CycleRecord {
            cycle_id: 4,
            cycle_start_unix_ns: 789,
            cycle_start_elapsed: Duration::ZERO,
            repeat_period: Duration::ZERO,
            idle_gap: Duration::ZERO,
            inventory_io: Duration::ZERO,
            buffer_wait: Duration::ZERO,
            cycle_total: Duration::ZERO,
            tags: Vec::new(),
            reader_result: Some(ReadResult {
                antenna_id: 0,
                read_rate: 0,
                total_read: 0,
            }),
            error: None,
        };
        let mut output = Vec::new();

        assert_eq!(write_record(&mut output, &record).unwrap(), 1);

        let row = String::from_utf8(output).unwrap();
        assert_eq!(row.trim_end().split(',').count(), 24);
        assert!(row.starts_with("4,"));
        assert!(row.contains(",OK,,0,"));
    }
}
