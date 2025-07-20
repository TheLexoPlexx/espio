#ESP-IO Software

For ESP32-S3-DevKitC-1

# CAN/TWAI
- 0x100 [0x01] update request
- 0x210 engine_bay_unit
- 0x222 engine_bay_unit abs sensors
  - [aa aa bb bb cc cc dd dd]
    - a-d wheel speed
- 0x310 kombiinstrument
  - [byz??????]
    - y & z == 1 = brake pedal active
- 0x444 dev_can_sender


- universal
  - [11] ecu online
  - [02 xx] confirm update1
    - xx update progress 0-255
  - [fy xx] error
    - y error state:
      - 0 warning
      - f critical
    - xx error number
