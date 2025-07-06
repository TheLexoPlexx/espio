#ESP-IO Software

For ESP32-S3-DevKitC-1

# CAN/TWAI
- 0x100 [0x01] update request
- 0x200 engine_bay_unit
  - 02 xx confirm updating
    -  xx Status 0-255
- 0x300 kombiinstrument
- 0x444 dev_can_sender


- universal
  - [11] ecu online
  - [02 xx] confirm update
    - xx update progress 0-255
  - [fy xx] error
    - y error state:
      - 0 warning
      - f critical
    - xx error number
