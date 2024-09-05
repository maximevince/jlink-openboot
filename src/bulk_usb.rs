use embassy_usb::driver::{Driver, Endpoint, EndpointError, EndpointIn, EndpointOut};
use embassy_usb::Builder;

/// This should be used as `device_class` when building the `UsbDevice`.

pub struct BulkClass<'d, D: Driver<'d>> {
    read_ep: D::EndpointOut,
    write_ep: D::EndpointIn,
}

impl<'d, D: Driver<'d>> BulkClass<'d, D> {
    pub fn new(builder: &mut Builder<'d, D>, max_packet_size: u16) -> Self {
        // Add a vendor-specific function (class 0xFF), and corresponding interface,
        // that uses our custom handler.
        let mut function = builder.function(0xFF, 0, 0);
        let mut interface = function.interface();
        let mut alt = interface.alt_setting(0xFF, 0xFF, 0xFF, None);
        let write_ep = alt.endpoint_bulk_in(max_packet_size);
        let read_ep = alt.endpoint_bulk_out(max_packet_size);
        drop(function);

        BulkClass { read_ep, write_ep }
    }

    /// Gets the maximum packet size in bytes.
    #[allow(dead_code)]
    pub fn max_packet_size(&self) -> u16 {
        // The size is the same for both endpoints.
        self.read_ep.info().max_packet_size
    }

    /// Writes a single packet into the IN endpoint.
    #[allow(dead_code)]
    pub async fn write_packet(&mut self, data: &[u8]) -> Result<(), EndpointError> {
        self.write_ep.write(data).await
    }

    // Writes data to the IN endpoint, splitting it into multiple packets if necessary.
    #[allow(dead_code)]
    pub async fn write_split(&mut self, data: &[u8]) -> Result<(), EndpointError> {
        Ok(for chunk in data.chunks(self.max_packet_size() as usize) {
            self.write_packet(chunk).await?;
        })
    }

    /// Reads a single packet from the OUT endpoint.
    #[allow(dead_code)]
    pub async fn read_packet(&mut self, data: &mut [u8]) -> Result<usize, EndpointError> {
        self.read_ep.read(data).await
    }

    /// Waits for the USB host to enable this interface
    #[allow(dead_code)]
    pub async fn wait_connection(&mut self) {
        self.read_ep.wait_enabled().await;
    }
}