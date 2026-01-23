//! ```markdown
//! /**
//!  *
//!  * ## Error Code Table:
//!  *
//!  * | #   | Hex Code | Identifier                                    | Description                                |
//!  * |-----|----------|----------------------------------------------|--------------------------------------------|
//!  * | 1   | 0x10     | CommandSuccess                               | Command succeeded.                         |
//!  * | 2   | 0x11     | command_fail                                 | Command failed.                            |
//!  * | 3   | 0x20     | mcu_reset_error                              | CPU reset error.                           |
//!  * | 4   | 0x21     | cw_on_error                                  | Turn on CW error.                          |
//!  * | 5   | 0x22     | antenna_missing_error                        | Antenna is missing.                        |
//!  * | 6   | 0x23     | write_flash_error                            | Write flash error.                         |
//!  * | 7   | 0x24     | read_flash_error                             | Read flash error.                          |
//!  * | 8   | 0x25     | set_output_power_error                       | Set output power error.                    |
//!  * | 9   | 0x31     | tag_inventory_error                          | Error occurred when inventory.             |
//!  * | 10  | 0x32     | tag_read_error                               | Error occurred when read.                  |
//!  * | 11  | 0x33     | tag_write_error                              | Error occurred when write.                 |
//!  * | 12  | 0x34     | tag_lock_error                               | Error occurred when lock.                  |
//!  * | 13  | 0x35     | tag_kill_error                               | Error occurred when kill.                  |
//!  * | 14  | 0x36     | no_tag_error                                 | There is no tag to be operated.            |
//!  * | 15  | 0x37     | inventory_ok_but_access_fail                 | Tag Inventoried but access failed.         |
//!  * | 16  | 0x38     | buffer_is_empty_error                        | Buffer is empty.                           |
//!  * | 17  | 0x3C     | nxp_custom_command_fail                      | NXP chips custom command failed.           |
//!  * | 18  | 0x40     | access_or_password_error                     | Access failed or wrong password.           |
//!  * | 19  | 0x41     | parameter_invalid                            | Invalid parameter.                         |
//!  * | 20  | 0x42     | parameter_invalid_wordCnt_too_long           | WordCnt is too long.                       |
//!  * | 21  | 0x43     | parameter_invalid_membank_out_of_range       | MemBank out of range.                      |
//!  * | 22  | 0x44     | parameter_invalid_lock_region_out_of_range   | Lock region out of range.                  |
//!  * | 23  | 0x45     | parameter_invalid_lock_action_out_of_range   | LockType out of range.                     |
//!  * | 24  | 0x46     | parameter_reader_address_invalid             | Invalid reader address.                    |
//!  * | 25  | 0x47     | parameter_invalid_AntennaID_out_of_range     | AntennaID out of range.                    |
//!  * | 26  | 0x48     | parameter_invalid_output_power_out_of_range  | Output power out of range.                 |
//!  * | 27  | 0x49     | parameter_invalid_frequency_region_out_of_range | Frequency region out of range.           |
//!  * | 28  | 0x4A     | parameter_invalid_baudrate_out_of_range      | Baud rate out of range.                    |
//!  * | 29  | 0x4B     | parameter_beeper_mode_out_of_range           | Buzzer behavior out of range.              |
//!  * | 30  | 0x4C     | parameter_epc_match_len_too_long             | EPC match is too long.                     |
//!  * | 31  | 0x4D     | parameter_epc_match_len_error                | EPC match length wrong.                    |
//!  * | 32  | 0x4E     | parameter_invalid_epc_match_mode             | Invalid EPC match mode.                    |
//!  * | 33  | 0x4F     | parameter_invalid_frequency_range            | Invalid frequency range.                   |
//!  * | 34  | 0x50     | fail_to_get_RN16_from_tag                    | Failed to receive RN16 from tag.           |
//!  * | 35  | 0x51     | parameter_invalid_drm_mode                   | Invalid DRM mode.                          |
//!  * | 36  | 0x52     | pll_lock_fail                                | PLL can not lock.                          |
//!  * | 37  | 0x53     | rf_chip_fail_to_response                     | No response from RF chip.                  |
//!  * | 38  | 0x54     | fail_to_achieve_desired_output_power         | Can’t achieve desired output power level.  |
//!  * | 39  | 0x55     | copyright_authentication_fail                | Can’t authenticate firmware copyright.     |
//!  * | 40  | 0x56     | spectrum_regulation_error                    | Spectrum regulation wrong.                 |
//!  * | 41  | 0x57     | output_power_too_low                         | Output power is too low.                   |
//!  * | 42  | 0xEE     | fail_to_get_rf_port_return_loss              | Failed to get RF port return loss.         |
//!  *
//!  * ## Usage:
//!  * Each of these error codes corresponds to a specific system state or fault condition and
//!  * should be used by developers to diagnose errors more effectively or to handle fault conditions appropriately in application code.
//!  *
//!  * @note Ensure to decode the hexadecimal value for better reference while debugging.
//!  */
//! ```

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ErrorCode {
    CommandSuccess = 0x10,
    CommandFail = 0x11,
    McuResetError = 0x20,
    CwOnError = 0x21,
    AntennaMissingError = 0x22,
    WriteFlashError = 0x23,
    ReadFlashError = 0x24,
    SetOutputPowerError = 0x25,
    TagInventoryError = 0x31,
    TagReadError = 0x32,
    TagWriteError = 0x33,
    TagLockError = 0x34,
    TagKillError = 0x35,
    NoTagError = 0x36,
    InventoryOkButAccessFail = 0x37,
    BufferIsEmptyError = 0x38,
    NxpCustomCommandFail = 0x3C,
    AccessOrPasswordError = 0x40,
    ParameterInvalid = 0x41,
    ParameterInvalidWordCntTooLong = 0x42,
    ParameterInvalidMembankOutOfRange = 0x43,
    ParameterInvalidLockRegionOutOfRange = 0x44,
    ParameterInvalidLockActionOutOfRange = 0x45,
    ParameterReaderAddressInvalid = 0x46,
    ParameterInvalidAntennaIdOutOfRange = 0x47,
    ParameterInvalidOutputPowerOutOfRange = 0x48,
    ParameterInvalidFrequencyRegionOutOfRange = 0x49,
    ParameterInvalidBaudrateOutOfRange = 0x4A,
    ParameterBeeperModeOutOfRange = 0x4B,
    ParameterEpcMatchLenTooLong = 0x4C,
    ParameterEpcMatchLenError = 0x4D,
    ParameterInvalidEpcMatchMode = 0x4E,
    ParameterInvalidFrequencyRange = 0x4F,
    FailToGetRn16FromTag = 0x50,
    ParameterInvalidDrmMode = 0x51,
    PllLockFail = 0x52,
    RfChipFailToResponse = 0x53,
    FailToAchieveDesiredOutputPower = 0x54,
    CopyrightAuthenticationFail = 0x55,
    SpectrumRegulationError = 0x56,
    OutputPowerTooLow = 0x57,
    FailToGetRfPortReturnLoss = 0xEE,
}

impl ErrorCode {
    pub fn from_hex(code: u8) -> Self {
        match code {
            0x10 => ErrorCode::CommandSuccess,
            0x11 => ErrorCode::CommandFail,
            0x20 => ErrorCode::McuResetError,
            0x21 => ErrorCode::CwOnError,
            0x22 => ErrorCode::AntennaMissingError,
            0x23 => ErrorCode::WriteFlashError,
            0x24 => ErrorCode::ReadFlashError,
            0x25 => ErrorCode::SetOutputPowerError,
            0x31 => ErrorCode::TagInventoryError,
            0x32 => ErrorCode::TagReadError,
            0x33 => ErrorCode::TagWriteError,
            0x34 => ErrorCode::TagLockError,
            0x35 => ErrorCode::TagKillError,
            0x36 => ErrorCode::NoTagError,
            0x37 => ErrorCode::InventoryOkButAccessFail,
            0x38 => ErrorCode::BufferIsEmptyError,
            0x3C => ErrorCode::NxpCustomCommandFail,
            0x40 => ErrorCode::AccessOrPasswordError,
            0x41 => ErrorCode::ParameterInvalid,
            0x42 => ErrorCode::ParameterInvalidWordCntTooLong,
            0x43 => ErrorCode::ParameterInvalidMembankOutOfRange,
            0x44 => ErrorCode::ParameterInvalidLockRegionOutOfRange,
            0x45 => ErrorCode::ParameterInvalidLockActionOutOfRange,
            0x46 => ErrorCode::ParameterReaderAddressInvalid,
            0x47 => ErrorCode::ParameterInvalidAntennaIdOutOfRange,
            0x48 => ErrorCode::ParameterInvalidOutputPowerOutOfRange,
            0x49 => ErrorCode::ParameterInvalidFrequencyRegionOutOfRange,
            0x4A => ErrorCode::ParameterInvalidBaudrateOutOfRange,
            0x4B => ErrorCode::ParameterBeeperModeOutOfRange,
            0x4C => ErrorCode::ParameterEpcMatchLenTooLong,
            0x4D => ErrorCode::ParameterEpcMatchLenError,
            0x4E => ErrorCode::ParameterInvalidEpcMatchMode,
            0x4F => ErrorCode::ParameterInvalidFrequencyRange,
            0x50 => ErrorCode::FailToGetRn16FromTag,
            0x51 => ErrorCode::ParameterInvalidDrmMode,
            0x52 => ErrorCode::PllLockFail,
            0x53 => ErrorCode::RfChipFailToResponse,
            0x54 => ErrorCode::FailToAchieveDesiredOutputPower,
            0x55 => ErrorCode::CopyrightAuthenticationFail,
            0x56 => ErrorCode::SpectrumRegulationError,
            0x57 => ErrorCode::OutputPowerTooLow,
            0xEE => ErrorCode::FailToGetRfPortReturnLoss,
            _ => unreachable!("Invalid error code: {:02X}", code),
        }
    }
}

