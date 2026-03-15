# Miner Firmware API Reference

Complete field inventory from live miner probes. Firmware implementations vary significantly.

## Vnish (S21)

Host: 10.36.1.102

### cgminer TCP (port 4028)

| Command | Auth | Key Fields |
|---------|------|------------|
| stats | None | GHS 5s, GHS av, temp1/2/3, temp2_1/2/3, temp3_1/2/3, temp_chip1/2/3, temp_pcb1/2/3, chain_consumption1/2/3, chain_vol1/2/3, chain_rate1/2/3, chain_rateideal1/2/3, freq_avg1/2/3, total_rate, total_rateideal, total_acn, chain_acn1/2/3, chain_hw1/2/3, fan1-4, fan_num, fan_mode, fan_pwm, state, chain_state1/2/3, chain_fault1/2/3, Type |
| summary | None | Elapsed, GHS 30m, Accepted, Rejected, Hardware Errors, Best Share, Fee Percent, Device Hardware% |
| devdetails | None | Name, Driver |
| devs | None | MHS 1m/5m/15m, Temperature |

### HTTP REST (port 80)

| Endpoint | Auth | Key Fields |
|----------|------|------------|
| GET /api/v1/summary | None | miner_type, miner_state, hr_stock, average_hashrate, instant_hashrate, hr_realtime, hr_nominal, hr_average, pcb_temp.min/max, chip_temp.min/max, power_consumption, power_usage, power_efficiency, hw_errors_percent, hw_errors, devfee_percent, chains[].{id, frequency, voltage, power_consumption, hashrate_ideal, hashrate_rt, hashrate_percentage, hr_error, hw_errors, pcb_temp.min/max, chip_temp.min/max, chip_statuses.red/orange/grey, status.state}, cooling.{fan_num, fans[].{id, rpm, status, max_rpm}, settings.mode.name, fan_duty}, pools[].{url, pool_type, user, status, asic_boost, diff, accepted, rejected, stale, ping}, psu.{temps.pfc_temp/llc1_temp/llc2_temp, psu_power_metering}, best_share, found_blocks |
| GET /api/v1/status | None | miner_state, miner_state_time, find_miner, restart_required, reboot_required, unlocked |

## Mara (S21 Imm)

Host: 10.36.1.103

### cgminer TCP (port 4028)

| Command | Auth | Key Fields |
|---------|------|------------|
| stats | None | GHS 5s, GHS av, rate_30m, frequency, temp1-4, temp2_1-4, temp_pcb1-4, temp_chip1-4, temp_pic1-4, chain_acn1-4, chain_rate1-4, chain_hw1-4, chain_acs1-4, freq1-4, fan_num, fan1-4, total_rateideal, total_acn, total_freqavg, miner_version, Type, BMMiner |
| summary | None | Elapsed, GHS 5s/av/30m, Accepted, Rejected, Hardware Errors, Best Share |
| devs | None | Temperature, MHS av/5s, Accepted, Rejected, Hardware Errors |
| version | None | BMMiner, Type, Model |
| pools | None | URL, User, Status, Accepted, Rejected, Difficulty Accepted/Rejected, Stratum Active, Diff |

Note: devdetails, config, coin, notify, edevs, estats commands are invalid.

### HTTP (port 80)

| Endpoint | Auth | Key Fields |
|----------|------|------------|
| GET /kaonsu/v1/spout | Digest (root:root) | ZIP archive containing: charts (timestamp, temperature_avg, power_consumption, hashrate_chains, fan_pwm_percent, ideal_hashrate), firmware_info (customerId, version, buildDate, gitCommit, buildNumber), registry_dump (PowerConsumption, PowerSource, sMinerType, sMinerTypeExtended, sMinerStatus, sHashboardType, sControlBoardType, FEATURES_KHEAVY_HASH, FEATURES_SCRYPT_HASH), psu (Vendor, Model, MinVoltage, MaxVoltage), messages (syslog), event (timestamp, level, category, message) |

## Stock (S21 XP)

Host: 10.36.1.51

### cgminer TCP (port 4028)

| Command | Auth | Key Fields |
|---------|------|------------|
| stats | None | GHS 5s, GHS av, rate_30m, frequency, temp1-3, temp2_1-3, temp_pcb1-4, temp_chip1-4, temp_pic1-4, chain_acn1-4, chain_rate1-4, chain_hw1-4, chain_acs1-4, freq1-4, total_freqavg, fan1-4, fan_num, total_rateideal, total_acn, total rate, miner_version, Type, BMMiner |
| summary | None | Elapsed, GHS 5s/av/30m, Accepted, Rejected, Hardware Errors, Best Share |
| version | None | BMMiner, API, Miner, CompileTime, Type |
| pools | None | URL, User, Status, Accepted, Rejected, Difficulty Accepted/Rejected, Stratum Active, Diff, Best Share |

Note: devdetails command returns error.

### CGI (port 80)

| Endpoint | Auth | Key Fields |
|----------|------|------------|
| GET /cgi-bin/stats.cgi | Digest (root:root) | elapsed, rate_5s, rate_30m, rate_avg, rate_ideal, rate_unit, chain_num, fan_num, fan[], hwp_total, miner-mode, freq-level, chain[].{index, freq_avg, rate_ideal, rate_real, asic_num, asic, temp_pic[], temp_pcb[], temp_chip[], hw, eeprom_loaded, sn, hwp, tpl} |
| GET /cgi-bin/summary.cgi | Digest (root:root) | elapsed, rate_5s/30m/avg/ideal, rate_unit, hw_all, bestshare, status[].{type, status, code, msg} |
| GET /cgi-bin/get_system_info.cgi | Digest (root:root) | minertype, nettype, macaddr, hostname, ipaddress, netmask, system_kernel_version, system_filesystem_version, firmware_type, serinum |
| GET /cgi-bin/get_miner_conf.cgi | Digest (root:root) | pools[].{url, user, pass}, bitmain-fan-ctrl, bitmain-fan-pwm, bitmain-work-mode |
| GET /cgi-bin/get_network_info.cgi | Digest (root:root) | nettype, netdevice, macaddr, ipaddress, netmask, conf_nettype, conf_hostname |

### HTTP (port 6060)

| Endpoint | Auth | Key Fields |
|----------|------|------------|
| GET /readvol | None | Plain text: current voltage, feedback voltage, power status |
| GET /power-{0,1,2} | None | Plain text: per-chain voltage in volts |
| GET /rate | None | Plain text: target hashrate in GH/s |
| GET /board_type | None | Plain text: board ID |
| GET /get_sn | None | Plain text: serial number |
| GET /productName | None | Plain text: model name |
| GET /warning | None | Plain text: warning status |
| GET /nonce | None | Plain text: per-chain, per-domain, per-ASIC nonce counts |
| GET /adc | None | Plain text: per-chain, per-ASIC ADC values (columns: d0-d3, sum, avg) |

## Braiins (S21+)

Host: 10.36.1.46

### cgminer TCP (port 4028)

| Command | Auth | Key Fields |
|---------|------|------------|
| devdetails | None | Chips, Cores, Frequency, Voltage, Model, Name |
| devs | None | MHS 5s/1m/5m/15m/av, Nominal MHS, Diff1 Work, Accepted, Rejected, Hardware Errors, Status, Device Elapsed |
| summary | None | MHS 5s/1m/5m/15m/24h/av, Elapsed, Accepted, Rejected, Hardware Errors, Best Share, Difficulty Accepted/Rejected |
| temps | None | Board, Chip |
| fans | None | RPM, Speed |
| tunerstatus | None | ApproximateChainPowerConsumption, ApproximateMinerPowerConsumption, PowerLimit, TunerMode, DynamicPowerScaling, TunerChainStatus[].{Status, Iteration, TunerRunning, StageElapsed, ApproximatePowerConsumptionWatt, PowerLimitWatt} |
| tempctrl | None | Mode, Target, Hot, Dangerous |
| version | None | BOSer, API |
| pools | None | URL, User, Status, Accepted, Rejected, Difficulty, Stratum Active, AsicBoost, Best Share |
| stats | None | Not useful on Braiins (mostly zeros) |

## LuxOS (S21 Pro)

Host: 192.168.4.11

### cgminer TCP (port 4028)

| Command | Auth | Key Fields |
|---------|------|------------|
| stats | None | GHS 5s, GHS av, rate_30m, frequency, freq1-4, total_freqavg, temp1-3, temp2_1-3, temp_max, temp_chip1-4, temp_pcb1-4, temp_pic1-4, chain_acn1-4, chain_rate1-4, chain_hw1-4, chain_acs1-4, fan1-4, fan_num, total_rateideal, total_acn, Type, Miner |
| summary | None | GHS 5s/1m/5m/15m/30m/24h/av, Elapsed, Accepted, Rejected, Hardware Errors, Best Share, Stale |
| devdetails | None | Voltage, Frequency, Chips, Cores, Model, Board, SerialNumber, Profile |
| devs | None | MHS 5s/1m/5m/15m/30m/60m/av, Nominal MHS, Temperature, Accepted, Rejected, Hardware Errors, Board, SerialNumber, Connector, Profile, IsRamping, IsUserShutdown |
| temps | None | BoardTopLeft, BoardTopRight, BoardMiddleLeft, BoardBottomLeft, METADATA (sensor labels, positions) |
| fans | None | RPM, Speed, FAN label, FANCTRL (FanMaxSpeed, FanMinSpeed, MinFans, PowerOffSpeed, QuietFanStartup) |
| power | None | Watts, PSU |
| voltageget | Parameter: board_id | Voltage, Board, IsOnBoard |
| frequencyget | Parameter: board_id | Freqs[] (65 per-chip values), Count |
| profiles | None | 21 profiles with: Profile Name, Step, Frequency, Voltage, Hashrate, Watts, IsDynamic, IsTuned |
| limits | None | FrequencyDefault/Min/Max, VoltageDefault/Min/Max/StepMin/StepMax, PowerTargetMin/Max, TemperatureMin/Max/Target/Hot/Panic, TemperatureChipMin/Max/Hot/Panic, FanSpeed limits, ATM limits, Health thresholds, TempSensor limits, PoolOpts limits, Update limits |
| config | None | ASC/PGA/Pool Count, network info (IP, Netmask, Gateway, MAC, DHCP, Hostname), Model, SerialNumber, ControlBoardType, PSU info (HwVersion, Label, IsPowerSupplyOn), power targets (ActualPowerTarget, IdealPowerTarget, PowerLimit, IsPowerTargetEnabled), Profile, IsTuning, ATM settings, Cooling mode, ImmersionMode, SystemStatus, CurtailMode, LEDs, Update settings, NameplateTHS, FeeStatus |
| events | None | Code, Target, Description, DocUrl, CreatedAt |
| tunerstatus | None | TunerRunning, HasSession |
| healthctrl | None | BadChipHealthThreshold, NumReadings |
| hashboardopts | None | NoPicProtectionMode, OvertempAutoRecovery |
| atm | None | Enabled, TempWindow, ChipTempWindow, MinProfile, MaxProfile, StartupMinutes, PostRampMinutes |
| poolopts | None | TimeoutSecs, MaxErrors, SmartSwitch, SmartSwitchSecs, BackoffOnError, HashOnDisconnect |
| coin | None | Hash Method, LP, Network Difficulty |
| version | None | LUXminer, API, Miner, CompileTime, Type |

## Notes

### Protocol Compatibility

- cgminer TCP (port 4028): All firmwares implement this with varying command support.
- HTTP APIs: Each firmware uses different endpoints and authentication schemes.
- Authentication: Varies by firmware (none, digest auth, or token-based).

### Temperature Reporting

Temperature field formats differ significantly:
- Vnish: Single sensor per board (temp1/2/3) plus detailed ranges (temp_chip, temp_pcb).
- Mara: Four values per board in range format (min-max).
- Stock: Four-value strings (min-avg-max1-max2) for temp_pcb/temp_chip/temp_pic.
- Braiins: Separate temps command returns Board and Chip temperatures.
- LuxOS: Four corner sensors (BoardTopLeft, BoardTopRight, BoardMiddleLeft, BoardBottomLeft) with metadata.

### Power Metrics

- Vnish: Per-chain power (chain_consumption1/2/3), total power via REST API.
- Mara: Total PowerConsumption in registry_dump ZIP.
- Stock: Voltage available via port 6060 endpoints, power calculated elsewhere.
- Braiins: ApproximateMinerPowerConsumption in tunerstatus.
- LuxOS: Total Watts in power command, per-board voltage available via voltageget.

### Voltage Reporting

- Vnish: Millivolts (chain_vol1/2/3).
- Mara: Not directly exposed in API.
- Stock: Volts as plain text (port 6060).
- Braiins: Volts (V) in devdetails.
- LuxOS: Volts (V) in devdetails and voltageget.

### Command Support Matrix

| Command | Vnish | Mara | Stock | Braiins | LuxOS |
|---------|-------|------|-------|---------|-------|
| stats | Yes | Yes | Yes | Limited | Yes |
| summary | Yes | Yes | Yes | Yes | Yes |
| devdetails | Yes | Invalid | Error | Yes | Yes |
| devs | Yes | Yes | No | Yes | Yes |
| version | No | Yes | Yes | Yes | Yes |
| pools | No | Yes | Yes | Yes | Yes |
| temps | No | No | No | Yes | Yes |
| fans | No | No | No | Yes | Yes |
| power | No | No | No | No | Yes |
| profiles | No | No | No | No | Yes |
| limits | No | No | No | No | Yes |
| config | No | Invalid | No | No | Yes |
| tunerstatus | No | No | No | Yes | Yes |
