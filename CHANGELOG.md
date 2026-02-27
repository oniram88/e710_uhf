# Changelog
All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

- - -
## 0.3.1 - 2026-02-27
#### Bug Fixes
- Change info level for tags count - (cd5a0d0) - Marino Bonetti
#### Documentation
- Add Cargo.toml info - (0536fe3) - Marino Bonetti

- - -

## 0.3.0 - 2026-02-20
#### Features
-  Split connector in Async e Sync capabilities with feature activation - (dfbfb06) - Marino Bonetti
- Define as duration the timeout - (8ee5d0f) - Marino Bonetti
- Better debug - (91446c4) - Marino Bonetti
- Reso configurabile il timeout - (4ceb2f3) - Marino Bonetti
- Ottimizzazione lettura pacchetti senza attendere timeout - (efcc005) - Marino Bonetti
- Completa reimplementazione async - (429db6b) - Marino Bonetti
- Implementazione versione async example - (efbe55c) - Marino Bonetti
#### Bug Fixes
- Rimozione commento - (4e20b35) - Marino Bonetti
- Rimozione codice inutile - (4c690d1) - Marino Bonetti
- Correzione lettura async - (82f21de) - Marino Bonetti
- Introduzione tempo di attesa per lettura chip - (6770ca2) - Marino Bonetti
- Correzione parsing pacchetti multipli - (6270c46) - Marino Bonetti
- Ottimizzazione lettura dati - (5578a9b) - Marino Bonetti
- Spostamento timeout lettura in modo sganciato dalla lettura del dato - (fadd25c) - Marino Bonetti
- Correzione iniziale codice con iterator - (bda6e88) - Marino Bonetti
- For common - (3657ef9) - Marino Bonetti
#### Documentation
- Add dev config - (bc86dfc) - Marino Bonetti
- Add test execution command - (92d9bd2) - Marino Bonetti
#### Tests
- Add coverage for sync - (8d11575) - Marino Bonetti
- Add coverage for async - (a8723bd) - Marino Bonetti
- Add coverage - (882a319) - Marino Bonetti
#### Refactoring
- Rewrite async read data connection - (52a9872) - Marino Bonetti
- Rename useful examples - (3a72c4f) - Marino Bonetti
- Remove commented old code - (2794c45) - Marino Bonetti
- Rimozione codice "non duplicato" per codice semplificato - (03a2164) - Marino Bonetti
- Copia delle funzione da async a sync - (8ff3e86) - Marino Bonetti
- Iniziale duplicazione codice - (5bed9e4) - Marino Bonetti
- Iniziale spostamento in sync del codice implementato - (5b0c0cf) - Marino Bonetti
- Predisposizione codice per unificazione connettore - (f47a9cf) - Marino Bonetti
#### Miscellaneous Chores
- Fix cog release procedure - (55dd9ef) - Marino Bonetti
- Code formatting and minor refactoring for clarity and consistency across modules - (6c9e898) - Marino Bonetti
- Formattazione - (916a1f5) - Marino Bonetti
- Add test Configuration - (dd22025) - Marino Bonetti

- - -

## 0.2.0 - 2026-02-12
#### Bug Fixes
- Corretto problema read blocking - (8820081) - Marino Bonetti
- For faster reading no phase - (5365734) - Marino Bonetti
#### Documentation
- Update documentation - (0a42ddb) - Marino Bonetti
#### Features
- Add examples with serial - (bba7921) - Marino Bonetti
- More debug and better read for Serial - (bd1916f) - Marino Bonetti
- Retry send if command failed - (651a73e) - Marino Bonetti
- Add `BeeperMode` enum and integrate `SetBeeperMode` command support - (54e5be6) - Marino Bonetti
#### Miscellaneous Chores
- **(version)** 0.2.0 - (830567c) - Marino Bonetti
- Add description and license metadata to `Cargo.toml` - (a60867b) - Marino Bonetti
- Exclude `docs/` directory from crate packaging in `Cargo.toml` - (52b82f5) - Marino Bonetti
#### Refactoring
- Replace `u8` with `PhaseStatus` in `CustomizeSessionTargetInventory` and related tests for improved readability and maintainability - (68bdc3b) - Marino Bonetti
- Introduce `PhaseStatus` enum and integrate into `FastSwitchAntInventory` command for improved clarity and maintenance - (f2c01f1) - Marino Bonetti
- Add `BeeperMode` import to `get_module_info.rs` for improved command integration - (ba553a2) - Marino Bonetti
#### Tests
- Add Beeper mode - (b0fe2a5) - Marino Bonetti

- - -

## 0.2.0 - 2026-02-12
#### Bug Fixes
- Corretto problema read blocking - (8820081) - Marino Bonetti
- For faster reading no phase - (5365734) - Marino Bonetti
#### Documentation
- Update documentation - (0a42ddb) - Marino Bonetti
#### Features
- Add examples with serial - (bba7921) - Marino Bonetti
- More debug and better read for Serial - (bd1916f) - Marino Bonetti
- Retry send if command failed - (651a73e) - Marino Bonetti
- Add `BeeperMode` enum and integrate `SetBeeperMode` command support - (54e5be6) - Marino Bonetti
#### Miscellaneous Chores
- Add description and license metadata to `Cargo.toml` - (a60867b) - Marino Bonetti
- Exclude `docs/` directory from crate packaging in `Cargo.toml` - (52b82f5) - Marino Bonetti
#### Refactoring
- Replace `u8` with `PhaseStatus` in `CustomizeSessionTargetInventory` and related tests for improved readability and maintainability - (68bdc3b) - Marino Bonetti
- Introduce `PhaseStatus` enum and integrate into `FastSwitchAntInventory` command for improved clarity and maintenance - (f2c01f1) - Marino Bonetti
- Add `BeeperMode` import to `get_module_info.rs` for improved command integration - (ba553a2) - Marino Bonetti
#### Tests
- Add Beeper mode - (b0fe2a5) - Marino Bonetti

- - -

Changelog generated by [cocogitto](https://github.com/cocogitto/cocogitto).