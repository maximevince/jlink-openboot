# JLink Open Bootloader

An open source bootloader for the JLink V9, written in Rust.  
This project is a work in progress and is barely functional.

## Notes

- The JLink V9 is based on the STM32F205RCT microcontroller. A 256KB flash and 96KB RAM, 64 pin LQFP package.

## USB VID/PID
1366:0101   SEGGER  J-Link ARM -> J-Link V9 in bootloader mode

## Memory map

- Flash:        0x08000000 - 0x0803FFFF
- Bootloader:   0x08000000 - 0x0800FFFF
- Signature:    0x0800B700 - 0x0800B800 (Can be filled w/ FFs)
- Config:       0x0800BF00 - 0x0800C000
- Application:  0x08010000 - 0x0803FFFF

The entire area bf00-c000 is digitally signed and the signature is at b700-b800. You can fill that with FFs

## J-Link V9 memory map
```
Type            Start Addr   End Addr       Length
----            ----------   ---------      ------
IVT             0x08000000   0x080001ff     0x200
Bootloader      0x08000200   0x080003ff     0x3e00
Signature       0x0800b700   0x0800b7ff     0x100
Serial Nr       0x0800bf00   0x0800bf03     4
Licenses        0x0800bf20   0x0800bfdf     0xc0
Config Data     0x0800c000   0x0800c0ff     0x100
User Licenses   0x0800c100   0x0800ffff     0x3f00
---
FW IVT          0x08010000   0x080101ff     0x200
FW Code         0x08010200   0x0803fffd     0x2fdfe
FW CRC16        0x0803fffe   0x0803ffff     2
---
Code in RAM     0x20000000   0x200030cf     0x30d0
Data in RAM     0x200030d0   0x20013fff     0x10f30
```

## Configuration area notes
- user licenses are stored in clear at address 0x0800c100,
- config licenses at 0x0800bf20
- serial number at 0x0800bf00
- 256 byte RSA digital signature at 0x0800b700 (The signature is derived from the hardware ID of your microcontroller using 65537 as the public key and a 2048 bit modulus which is stored in the firmware)

## USB interface

Device descriptor:
- bcdUSB: 0x0200
- bDeviceClass: 0x00 (Defined at interface level)
- bDeviceSubClass: 0x00
- bDeviceProtocol: 0x00
- bMaxPacketSize0: 0x40 (64 bytes)
- idVendor: 0x1366 (SEGGER)
- idProduct: 0x0101 (J-Link)
- bcdDevice: 0x0100
- iManufacturer: 1
- iProduct: 2
- iSerialNumber: 3
- bNumConfigurations: 1

Configuration descriptor:
- bDescriptorType: 0x04 (Interface)
- bInterfaceNumber: 0
- bInterfaceClass: 0xFF (Vendor specific)
- bInterfaceSubClass: 0xFF
- bInterfaceProtocol: 0xFF
- iInterface: 0

Endpoints:
- 0x81: EP 1 IN, Bulk, MaxPacketSize 64
- 0x01: EP 1 OUT, Bulk, MaxPacketSize 64

## Bootloader command list
Following commands are supported by the original bootloader:
1. cmd_01_version [supported]
1. cmd_04_get_info
1. cmd_05_set_speed
1. cmd_06_update_firmware [supported]
1. cmd_e6_read_config_bf00
1. cmd_ed_get_caps_ex
1. cmd_f0_get_hw_version
1. cmd_fe_read_emu_mem



