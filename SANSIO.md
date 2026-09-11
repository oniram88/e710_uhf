# Piano di migrazione a un'architettura sans-I/O

## Scopo della review

La review parte da `src/frame/command.rs` e include i punti che ne definiscono il confine I/O: `src/frame.rs`, `src/tag.rs`, `src/error_references.rs`, `src/connector/sync.rs`, `src/connector/async_impl.rs` e gli iteratori dei tag.

L'obiettivo non è rendere asincrono il parser. Sans-I/O significa che il nucleo del protocollo:

- riceve byte già letti e produce eventi/risposte;
- produce byte da scrivere, senza scriverli;
- non conosce `Read`, `Write`, Tokio, socket, seriali, sleep, timeout o clock di sistema;
- mantiene esplicitamente lo stato necessario tra chunk parziali;
- usa lo stesso codice con trasporti sync, async, TCP, seriale e mock.

## Valutazione sintetica

La base è adatta a una migrazione incrementale: l'encoding dei comandi e il checksum sono già funzioni pure, i trasporti sono generici e ci sono fixture reali per risposte singole e inventari multi-frame. La suite corrente passa con tutte le feature (62 test).

Il confine è però invertito rispetto a un design sans-I/O: sono i loop sync/async a possedere l'accumulo dei byte e a interrogare ripetutamente un parser stateless. `Command::from_bytes` deve quindi indovinare se il buffer è completo, e `try_parsing_results` usa qualsiasi errore come segnale di incompletezza. Questo rende indistinguibili “mancano byte”, checksum errato, risposta inattesa e payload invalido.

## Problemi trovati

### Priorità P0 — correttezza e assenza di panic

1. **Gli errori definitivi vengono scambiati per input incompleto.**
   `try_parsing_results` restituisce `None` per ogni `Err` di `Command::from_bytes`. Un checksum errato, un comando diverso da quello atteso o un payload malformato fanno quindi continuare la lettura fino al timeout. Di conseguenza, il ramo di retry per `InvalidPacketOrder` nei connector è di fatto irraggiungibile attraverso il normale percorso di lettura.

2. **Il parser può andare in panic con byte ricevuti dal dispositivo.**
   `parse_response!`, i decoder di firmware/temperatura/frequenza, `parse_tag_response` e `Tag::from_raw*` indicizzano slice senza verificarne la lunghezza. Sono inoltre presenti `unwrap()` sul footer inventario e su `result`. Un frame formalmente completo ma con payload corto può terminare il processo.

3. **Codici e frequenze sconosciuti causano panic.**
   `ErrorCode::from_hex` usa `unreachable!` per byte sconosciuti; `get_frequency` e `get_param` usano `panic!`. Sono dati provenienti rispettivamente dal wire e dall'API pubblica e devono produrre errori tipizzati.

4. **`FastSwitchAntInventory` con più di otto antenne può andare in panic.**
   L'espressione `8 - antennas.len()` va in underflow. Mancano anche controlli sugli ID antenna, potenza, range di frequenza e altri parametri del comando.

### Priorità P1 — bug di protocollo e framing

5. **La risposta a `SetOutputPower` ha il tipo sbagliato.**
   Il comando `0x76` produceva `CommandResult::Reset(...)`; nell'enum mancava una variante `SetOutputPower`. Questo poteva nascondere un errore di configurazione e rompere il matching tipizzato del chiamante. **Risolto nella Fase 0.**

6. **Header e indirizzo non vengono validati.**
   `try_split_in_base_frame_parts` non verifica `FRAME_HEADER` e non espone/controlla l'indirizzo RS-485. Un frame con struttura compatibile può essere accettato anche se destinato a un altro reader.

7. **Il checksum del frame singolo usa tutto il buffer, non la lunghezza dichiarata.**
   `Command::from_bytes` calcola il checksum su `raw[..raw.len() - 1]`, mentre il checksum estratto è quello a `length + 1`. Se il buffer contiene due frame concatenati o byte successivi, la verifica usa limiti incoerenti. Il parser deve consumare esattamente `length + 2` byte e lasciare il resto nel buffer.

8. **Byte residui e rumore vengono persi o ignorati senza una policy esplicita.**
   I connector creano un buffer locale per ogni comando e lo eliminano appena ottengono un risultato. `split_packets` salta i byte prima di `0xA0` e ignora una coda incompleta. Questo può occultare corruzione oppure perdere l'inizio della risposta seguente.

9. **L'inventario viene riconosciuto con euristiche fragili.**
   Il footer è identificato soprattutto da `length == 0x0A`; solo il command byte del primo frame viene confrontato con il comando inviato. I command byte e gli indirizzi dei frame successivi sono ignorati. Un insieme misto di frame può quindi essere accettato, mentre un errore può sembrare una risposta ancora incompleta.

10. **Il buffer può crescere senza un limite di protocollo.**
    In presenza di rumore o flusso continuo non conclusivo, i reader continuano ad accumulare. Il timeout sync, inoltre, viene reimpostato a ogni chunk e non viene controllato nel ramo `WouldBlock`. Servono limite del frame, limite del buffer e una policy di resync dichiarata.

11. **Il retry è ricorsivo, illimitato e rispedisce il comando.**
    `send_and_read_command` richiama sé stesso su risposta fuori ordine. Può consumare stack/risorse e ripetere comandi con effetti collaterali. Il core deve segnalare l'evento; retry, numero massimo e idempotenza sono policy del chiamante/adapter.

### Priorità P2 — modello e manutenibilità

12. **`CommandResult` ha due canali di errore.**
    Si ottiene `Result<CommandResult, FrameError>`, ma quasi ogni variante contiene un altro `Result<_, FrameError>`. Il chiamante deve gestire combinazioni incoerenti come errore esterno, errore interno o variante inattesa. È preferibile `Result<Response, ProtocolError>`, con gli errori del device modellati nel solo `ProtocolError` o in una risposta dedicata.

13. **La decodifica dei tag legge il clock di sistema.**
    `Tag::from_raw*` chiama `Utc::now()` e `SystemTime::now()`. Questo rende il core non deterministico e non strettamente sans-I/O. Il parser deve produrre `DecodedTag`; timestamp e `Tag` pubblico vanno creati al confine, usando un clock iniettato o un timestamp fornito dall'adapter.

14. **Logging e semantica sono mescolati nel parser.**
    Il core emette log durante il decode. È meglio produrre errori/eventi ricchi e lasciare il logging agli adapter, così i test non dipendono da effetti collaterali.

15. **Le implementazioni sync e async duplicano la stessa macchina di lettura.**
    I due loop differiscono già nel trattamento di EOF, `WouldBlock` e timeout. Con un decoder incrementale condiviso gli adapter rimangono piccoli e la parità di comportamento diventa verificabile.

16. **Alcune semantiche pubbliche sono ambigue.**
    `SetWorkAntenna` accetta un indice zero-based, mentre `GetWorkAntenna` aggiunge uno; le frequenze sono `f64` confrontati per uguaglianza esatta; `Spectrum::CUSTOM` restituisce per ora `(0.0, 0.0)`. Il `Display` errato di `Reset` è stato corretto nella Fase 0; le altre scelte vanno fissate o documentate prima di stabilizzare la nuova API.

17. **Qualità statica.**
    `cargo clippy --all-targets --all-features -- -D warnings` fallisce attualmente con 33 segnalazioni complessive. Molte sono cosmetiche, ma conviene avere Clippy verde come guardrail della migrazione.

## Architettura obiettivo

Separare il protocollo in quattro livelli, con dipendenze solo dall'alto verso il basso:

```text
API di alto livello / setup / iteratori
                |
adapter sync e async: Read/Write, timeout, retry, timestamp, log
                |
ResponseDecoder: stato della transazione e inventario multi-frame
                |
FrameCodec + CommandEncoder: byte, framing, checksum, validazione
```

Una possibile organizzazione dei file:

```text
src/protocol/
  mod.rs
  command.rs       # Command, tipi validati, encoding del payload
  frame.rs         # RawFrame, encode_frame, decoder incrementale
  response.rs      # risposta semantica e accumulatore inventario
  error.rs         # EncodeError, DecodeError, ProtocolError
src/connector/
  sync.rs          # solo I/O bloccante e policy temporali
  async_impl.rs    # solo I/O Tokio e policy temporali
```

Non è necessario rendere il crate `no_std`: sans-I/O richiede l'assenza di operazioni I/O nel core, non l'assenza della standard library.

### Contratti suggeriti

Le firme definitive possono cambiare, ma devono conservare queste responsabilità:

```rust
pub fn encode_request(
    command: &Command,
    address: ReaderAddress,
    out: &mut Vec<u8>,
) -> Result<(), EncodeError>;

pub struct FrameDecoder { /* buffer e policy di resync */ }

impl FrameDecoder {
    pub fn push(&mut self, bytes: &[u8]) -> Result<(), DecodeError>;
    pub fn next_frame(&mut self) -> Result<Option<RawFrame>, DecodeError>;
}

pub struct ResponseDecoder { /* comando atteso e stato inventario */ }

pub enum ResponseProgress {
    NeedMore,
    Tag(DecodedTag),
    Complete(Response),
}

impl ResponseDecoder {
    pub fn accept(&mut self, frame: RawFrame)
        -> Result<ResponseProgress, ProtocolError>;
}
```

`RawFrame` deve contenere almeno indirizzo, command byte e payload già verificati. Il decoder di frame deve:

- cercare o pretendere `0xA0` secondo una policy documentata;
- attendere Head + Len senza considerarlo un errore;
- calcolare `frame_len = len + 2` con aritmetica checked;
- imporre lunghezze minima e massima;
- validare il checksum solo sui byte del frame;
- consumare un solo frame e preservare tutti i byte residui;
- distinguere `NeedMore` da un errore definitivo.

`ResponseDecoder` deve conoscere un solo comando in-flight. Per i comandi normali completa al primo frame valido. Per `0x8A` e `0x8B` emette ogni tag quando arriva e completa solo sul footer specifico del comando. In questo modo il numero di tag non determina la dimensione del buffer.

## Operazioni da eseguire

### Fase 0 — congelare il comportamento e correggere i bug certi

- [x] Aggiungere fixture/golden test per ogni comando supportato, sia encoding sia decoding.
- [x] Aggiungere `CommandResult::SetOutputPower` e mappare `0x76` sulla variante corretta.
- [x] Correggere il `Display` di `Reset` e le altre stringhe palesemente errate senza cambiare il wire format.
- [ ] Aggiungere test che dimostrino i panic attuali usando payload corti, error code ignoto, frequenza invalida e più di otto antenne; trasformarli poi in normali `Err`.
- [ ] Documentare e testare la convenzione dell'antenna: scegliere zero-based sul wire e, preferibilmente, anche nell'API; in alternativa introdurre un newtype che renda esplicita la conversione.
- [ ] Decidere il comportamento reale di `Spectrum::CUSTOM`; fino all'implementazione, restituire `UnsupportedResponse` anziché dati fittizi.

**Criterio di uscita:** nessun input pubblico o byte ricevuto può provocare panic; tutte le fixture esistenti continuano a produrre gli stessi valori validi.

### Fase 1 — introdurre tipi ed errori del protocollo

- [ ] Rendere `Session`, `Target`, `BeeperMode`, `PhaseStatus`, `RfLinkProfile` e `Spectrum` `Copy + Eq` dove appropriato e centralizzare `TryFrom<u8>`.
- [ ] Sostituire `ErrorCode::from_hex -> ErrorCode` con `TryFrom<u8>` oppure aggiungere `ErrorCode::Unknown(u8)`.
- [ ] Sostituire le frequenze `f64` nel wire model con un tipo esatto (`FrequencyCode`, oppure un newtype in kHz/mezzi MHz). La conversione di visualizzazione in `f64` può restare all'esterno.
- [ ] Introdurre `ReaderAddress` e rendere l'indirizzo configurabile; mantenere `0x01` come default compatibile.
- [ ] Definire errori separati: `EncodeError` per parametri non validi, `DecodeError` per framing, `ProtocolError` per payload/comando/risposta device.
- [ ] Eliminare progressivamente i `Result` annidati da `CommandResult`, introducendo `Response` e una conversione temporanea verso la vecchia enum.
- [ ] Far restituire `Result` all'encoder. Validare almeno: massimo otto antenne, ID `0..=7`, potenza `0..=33`, range e ordine delle frequenze, valori ammessi di repeat/stay se definiti dal manuale.

**Criterio di uscita:** tutte le conversioni da/verso byte sono totali oppure restituiscono un errore tipizzato; nessun `panic!`, `unreachable!` o indexing unchecked è raggiungibile da dati esterni.

### Fase 2 — creare il codec incrementale dei frame

- [ ] Spostare checksum e costruzione del frame in `protocol/frame.rs` come funzioni pure.
- [ ] Introdurre `RawFrame` con `address`, `command`, `payload` e, solo se utile al debug, i byte raw.
- [ ] Implementare `FrameDecoder` con buffer persistente e metodo `push`/`next_frame`.
- [ ] Preservare i byte dopo il primo frame completo: due frame nello stesso chunk devono produrre due chiamate riuscite a `next_frame`.
- [ ] Definire una policy di resync. Suggerimento: scartare rumore fino al prossimo header, ma emettere un evento/contatore diagnostico; dopo checksum errato scartare solo il candidato corrente e cercare il successivo header.
- [ ] Imporre `MAX_FRAME_LEN` in base al byte Len e un limite separato al buffer interno.
- [ ] Rimuovere `try_split_in_base_frame_parts`, `split_packets` e la verifica checksum basata sull'intero buffer solo dopo aver portato tutte le fixture sul nuovo decoder.

**Criterio di uscita:** il risultato è identico fornendo un frame in un chunk, un byte per volta o in qualsiasi partizione; frame concatenati e rumore non causano perdita silenziosa di dati.

### Fase 3 — separare la semantica della risposta dal framing

- [ ] Sostituire il macro `parse_response!` con funzioni esplicite che validino la lunghezza attesa prima di leggere il payload.
- [ ] Implementare un decoder dedicato per ogni forma di risposta supportata. Ogni decoder deve specificare lunghezza, range e codici di errore ammessi.
- [ ] Verificare address e command byte per **ogni** frame, non solo per il primo.
- [ ] Implementare `ResponseDecoder`/`PendingResponse`, creato con il comando inviato.
- [ ] Per risposte singole, restituire immediatamente `Complete(Response)`.
- [ ] Per inventario, emettere `Tag(DecodedTag)` per ogni frame tag e `Complete(InventorySummary)` soltanto dopo un footer valido per `0x8A` o `0x8B`.
- [ ] Validare esplicitamente i footer: sette byte di payload per il fast switching; campi antenna/read-rate/total per customize; forma distinta del frame di errore antenna.
- [ ] Definire cosa fare con una risposta di un altro comando: errore `UnexpectedCommand`, coda unsolicited oppure callback. Non rispedire automaticamente il comando nel core.
- [ ] Decidere se conservare l'ordine wire dei tag. L'attuale `reverse()` restituisce i tag in ordine inverso; mantenerlo solo se è un requisito esplicito e coperto da test.

**Criterio di uscita:** il core sa sempre distinguere `NeedMore`, evento intermedio, completamento ed errore definitivo; inventari molto grandi sono elaborati con memoria limitata.

### Fase 4 — rendere pura la decodifica dei tag

- [ ] Introdurre `DecodedTag` senza timestamp e rendere i decoder `Result<DecodedTag, TagDecodeError>`.
- [ ] Controllare prima le lunghezze minime: almeno FreqAnt + PC + RSSI, e due byte aggiuntivi quando la phase è attiva.
- [ ] Rendere fallibile la conversione del parametro frequenza.
- [ ] Spostare la creazione di `received_at_utc` e `received_at_ns` nell'adapter o in `Tag::from_decoded(decoded, received_at)`.
- [ ] Per test deterministici, accettare un `Clock` iniettato oppure passare direttamente un `ReceivedAt` catturato dal connector.
- [ ] Valutare se il timestamp debba essere catturato alla ricezione del chunk o all'estrazione del singolo frame e documentare la scelta.

**Criterio di uscita:** dati e stato uguali producono sempre gli stessi eventi del core, senza accesso al clock o al logger.

### Fase 5 — ridurre i connector ad adapter I/O

- [ ] Aggiungere il decoder persistente a `Connector`, così gli eventuali byte residui sopravvivono tra due operazioni.
- [ ] Estrarre una piccola API comune che alimenti `FrameDecoder` e `ResponseDecoder`; sync e async devono differire solo nel modo in cui leggono/scrivono e applicano timeout.
- [ ] Nel connector sync, controllare la deadline anche dopo `WouldBlock`; rimuovere sleep fissi quando il trasporto è già bloccante o ha timeout configurato.
- [ ] Nel connector async, lasciare a `tokio::time::timeout` la deadline e non aggiungere una latenza fissa di 1300 µs a ogni read salvo requisito hardware misurato.
- [ ] Trattare `Ok(0)` come EOF in modo coerente tra sync e async.
- [ ] Mappare separatamente `io::Error`, timeout e `ProtocolError` in `ConnectorError`.
- [ ] Sostituire il retry ricorsivo con una policy iterativa, limitata e configurabile. Per default non ritentare automaticamente comandi non dichiarati idempotenti.
- [ ] Far consumare agli iteratori gli eventi `Tag` del core; rimuovere gli `unreachable!` e restituire una variante d'errore se arriva una risposta incompatibile.

**Criterio di uscita:** le stesse sequenze di chunk producono lo stesso risultato via mock sync e mock async; nessun codice di parsing è duplicato nei connector.

### Fase 6 — compatibilità e API pubblica

- [ ] Conservare temporaneamente `Frame::new`, `CommandResult` e `send_and_read_command` come facade sopra il nuovo core.
- [ ] Aggiungere conversioni tra `Response` e il vecchio `CommandResult`, marcando deprecate solo le API che devono realmente sparire.
- [ ] Documentare il flusso sans-I/O pubblico con un esempio che codifica una richiesta, alimenta chunk e gestisce gli eventi senza socket.
- [ ] Aggiornare gli esempi TCP, RS-232 sync e async per usare gli adapter, non il parser direttamente.
- [ ] Fare il breaking change finale solo in una major/minor coerente con la policy SemVer del crate.

**Criterio di uscita:** gli utenti esistenti hanno un percorso di migrazione esplicito e gli utenti avanzati possono usare il core senza dipendenze da un runtime I/O.

### Fase 7 — verifica e robustezza

- [ ] Eseguire `cargo fmt --check`.
- [ ] Portare a verde `cargo clippy --all-targets --all-features -- -D warnings`.
- [ ] Eseguire `cargo test` e `cargo test --all-features` in CI.
- [ ] Aggiungere test tabellari per ogni possibile punto di split delle fixture multi-frame.
- [ ] Aggiungere test per due risposte concatenate, rumore iniziale, header nel payload, Len minimo/massimo, checksum errato, address errato, comando errato, payload corto e footer mancante.
- [ ] Aggiungere test di parità sync/async alimentando gli stessi chunk.
- [ ] Aggiungere property test: `decode(encode(command))` per tutti i comandi validi e “il decoder non va mai in panic” per byte arbitrari.
- [ ] Aggiungere fuzz target per `FrameDecoder::push/next_frame` e `ResponseDecoder::accept`; usare un limite di memoria nel corpus.
- [ ] Eseguire Miri sui test puri se compatibile e misurare l'allocazione su inventari grandi.

**Criterio di uscita:** nessun panic con input arbitrario, memoria limitata, suite e lint verdi con e senza feature `async`.

## Sequenza di commit consigliata

1. Test di caratterizzazione e correzione `SetOutputPower`.
2. Errori fallibili, validazione comandi e rimozione dei panic da dati esterni.
3. `RawFrame` + `FrameDecoder` incrementale con test di chunking.
4. `ResponseDecoder` per risposte singole.
5. Stato inventario multi-frame ed eventi tag/footer.
6. `DecodedTag` puro e clock spostato nell'adapter.
7. Adozione nel connector sync.
8. Adozione nel connector async e test di parità.
9. Facade di compatibilità, documentazione e deprecazioni.
10. Property test/fuzzing e rimozione del vecchio parser.

Ogni commit dovrebbe mantenere verdi i test, evitando una riscrittura unica difficile da revisionare.

## Definition of done

La migrazione è completa quando:

- il modulo `protocol` non importa `std::io`, Tokio, socket/serialport, funzioni di sleep, `Instant`, `SystemTime`, `Utc::now` o API di logging;
- encoding e decoding sono fallibili e non vanno in panic per input esterno;
- il decoder accetta chunk arbitrari, conserva i residui e distingue incompletezza da errore;
- ogni frame valida header, lunghezza, address, command e checksum;
- gli inventari producono tag incrementalmente e terminano soltanto con un footer valido;
- timeout, retry, logging e timestamp sono policy degli adapter;
- sync e async condividono interamente codec e macchina di stato;
- API legacy e percorso di migrazione sono documentati;
- format, Clippy, test standard, test con tutte le feature e fuzz/property test risultano verdi.
