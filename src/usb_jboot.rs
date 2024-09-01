use embassy_usb::driver::{Driver, Endpoint, EndpointIn, EndpointOut};
use defmt::*;
use embassy_time::Timer;
use static_cell::StaticCell;

pub struct JBoot<'a, D: Driver<'a>> {
    read_ep: D::EndpointOut,
    write_ep: D::EndpointIn,
    big_buf: &'static StaticCell<[u8; 0x500]>,
}


impl<'a, D> JBoot<'a, D>
where
    D: Driver<'a>,
{
    pub fn new(read_ep: D::EndpointOut, write_ep: D::EndpointIn, big_buf: &'static StaticCell<[u8; 1280]>) -> Self {
        Self {
            read_ep,
            write_ep,
            big_buf,
        }
    }

    pub async fn run(&mut self) {
        let mut buf = [0u8; 64];

        loop {
            self.read_ep.wait_enabled().await;
            // info!("Connected");

            // Read data from the Bulk OUT endpoint
            match self.read_ep.read(&mut buf).await {
                Ok(received_len) => {
                    self.process_data(&buf[..received_len]).await;
                }
                Err(_) => {
                    // Handle errors here, maybe reset or log
                    warn!("Error reading from Bulk OUT endpoint");
                }
            }
        }
    }

    async fn cmd_01_version(&mut self) {
        info!("Command 0x01: version");
        // hardcoded hex response: 4a2d4c696e6b20563920636f6d70696c6564204f637420313220323031322042544c202020202000436f7079726967687420323030332d32303132205345474745523a207777772e7365676765722e636f6d
        let response = [
            0x4A, 0x2D, 0x4C, 0x69, 0x6E, 0x6B, 0x20, 0x56, 0x39, 0x20, 0x63, 0x6F, 0x6D, 0x70, 0x69, 0x6C,
            0x65, 0x64, 0x20, 0x4F, 0x63, 0x74, 0x20, 0x31, 0x32, 0x20, 0x32, 0x30, 0x31, 0x32, 0x20, 0x42,
            0x54, 0x4C, 0x20, 0x20, 0x20, 0x20, 0x20, 0x00, 0x43, 0x6F, 0x70, 0x79, 0x72, 0x69, 0x67, 0x68,
            0x74, 0x20, 0x32, 0x30, 0x30, 0x33, 0x2D, 0x32, 0x30, 0x31, 0x32, 0x20, 0x53, 0x45, 0x47, 0x47,
            0x45, 0x52, 0x3A, 0x20, 0x77, 0x77, 0x77, 0x2E, 0x73, 0x65, 0x67, 0x67, 0x65, 0x72, 0x2E, 0x63,
            0x6F, 0x6D, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00 ];
        // first send the length of the response as 2-byte little endian
        self.write_ep.write(&[response.len() as u8, (response.len() >> 8) as u8]).await.ok();

        // then send response data in chunks of 64 bytes, including the last chunk even if it's not full
        for chunk in response.chunks(64) {
            info!("Sending chunk of {} bytes", chunk.len());
            // check for error, after sending the chunk
            match self.write_ep.write(chunk).await {
                Ok(_) => {},
                Err(_) => {
                    info!("Error sending chunk");
                    break;
                }
            }
        }
    }

    async fn cmd_04_get_info(&mut self) {
        info!("Command 0x04: get info");
        // send response: single 0x00 byte
        self.write_ep.write(&[0x00]).await.ok();
    }

    async fn cmd_06_update_firmware(&mut self) {
        info!("Command 0x06: update firmware");
        // send response: single 0x00 byte
        self.write_ep.write(&[0x00]).await.ok();

        // read 2 bytes: length
        let mut len_buf_1 = [0; 2];
        self.read_ep.read(&mut len_buf_1).await.ok();

        // interpret 2 bytes as uint16_t
        let len = u16::from_le_bytes(len_buf_1);
        let mut total_firmware_len: u32 = len as u32;
        info!("len: 0x{:x}", len);
        if len > 0x8000 {
            let mut len_buf_2 = [0; 2];
            self.read_ep.read(&mut len_buf_2).await.ok();
            let len2 = u16::from_le_bytes(len_buf_2);
            info!("len2: 0x{:x}", len2);
            let len4 = u32::from_le_bytes([len_buf_1[0], len_buf_1[1], len_buf_2[0], len_buf_2[1]]);
            info!("len4: 0x{:x}", len4);
            total_firmware_len = (len4 & 0x3fff) | (len4 >> 16) << 14;
        }

        // Now read 0x500 bytes of data into a buffer
        let len_to_read = 0x500;
        let big_buf: &'static mut [u8; 0x500] = self.big_buf.init([0; 0x500]);
        // read in chunks of 64 bytes
        let mut offset: u32 = 0;
        while offset < len_to_read {
            let chunk_len = core::cmp::min(64, (len_to_read - offset) as usize);
            let mut chunk = [0; 64];
            self.read_ep.read(&mut chunk).await.ok();
            for i in 0..chunk_len {
                big_buf[(offset + i as u32) as usize] = chunk[i];
            }
            offset += chunk_len as u32;
        }

        // Find "J-Link V9" at offset 0x210
        let target: &[u8] = b"J-Link V9";
        // use find_substring starting from offset 0x210 inside big_buf, with a max_range of 10
        let found = match find_substring(&big_buf[0x210..], target, 10) {
            Some(_) => true,
            None => false,
        };

        if !found {
            info!("J-Link V9 string not found");
            return;
        } else {
            info!("J-Link V9 string found");
        }

        // Check if the firmware length is exactly 0x30000
        if total_firmware_len == 0x30000 {
            info!("Firmware length is 0x30000");
        } else {
            info!("Firmware length is not 0x30000");
            return;
        }

        // read chunks of 64 bytes until we have read final_len bytes
        // offset should be 0x500 from the previous read
        while offset < total_firmware_len {
            let chunk_len = core::cmp::min(64, (total_firmware_len - offset) as usize);
            let mut chunk = [0; 64];
            self.read_ep.read(&mut chunk).await.ok();
            // throw away the data for now
            // for i in 0..chunk_len {
            //     big_buf[(offset + i as u32) as usize] = chunk[i];
            // }

            // TODO: write the data to flash

            offset += chunk_len as u32;
            debug!("offset: 0x{:x}", offset);
        }

        // Send ACK
        debug!("Sending ACK after reading firmware");
        self.write_ep.write(&[0x00]).await.ok();

        // delay 100ms
        Timer::after_millis(100).await;

        // Reboot
        cortex_m::peripheral::SCB::sys_reset();
    }

    async fn process_data(&mut self, data: &[u8]) {
        // Following commands are supported by the original V9 bootloader:
        // - cmd_01_version
        // - cmd_04_get_info
        // - cmd_05_set_speed
        // - cmd_06_update_firmware
        // - cmd_e6_read_config_bf00
        // - cmd_ed_get_caps_ex
        // - cmd_f0_get_hw_version
        // - cmd_fe_read_emu_mem

        let count = data.len();
        if count == 1 {
            match data[0] {
                0x01 => {
                    self.cmd_01_version().await;
                }
                0x04 => {
                    self.cmd_04_get_info().await;
                }
                0x06 => {
                    self.cmd_06_update_firmware().await;
                }
                _ => {
                    warn!("Unknown command 0x{:02x}", data[0]);
                    //self.write_ep.write(&[0xFF]).await.ok();
                }
            }
        } else {
            warn!("Got unknown data with len: {} bytes", count);
        }
    }
}

fn find_substring(slice: &[u8], target: &[u8], max_range: usize) -> Option<usize> {
    // Lengths of the slice and the target
    let slice_len = slice.len();
    let target_len = target.len();

    // Calculate the effective search length based on the max_range
    let search_len = if max_range < slice_len {
        max_range
    } else {
        slice_len
    };

    // Early return if the target is longer than the search length
    if target_len > search_len {
        return None;
    }

    // Iterate through the slice within the max range
    for i in 0..=(search_len - target_len) {
        // Compare the substring in the slice with the target
        if &slice[i..i + target_len] == target {
            return Some(i);  // Return the starting index of the match
        }
    }

    None // Return None if no match is found
}

