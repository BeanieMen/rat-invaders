pub struct SshRatatui {
    handle: russh::server::Handle,
    channel: russh::ChannelId,

    width: u16,
    height: u16,
}

impl SshRatatui {
    pub fn new(
        handle: russh::server::Handle,
        channel: russh::ChannelId,

        width: u16,
        height: u16,
    ) -> Self {
        Self {
            handle,
            channel,
            width,
            height,
        }
    }

    pub fn write(&self, data: &[u8]) -> std::io::Result<()> {
        let handle = self.handle.clone();
        let channel = self.channel;
        let data = data.to_vec();

        tokio::spawn(async move {
            if let Err(e) = handle.data(channel, data).await {
                eprintln!("SSH write failed: {e:?}");
            }
        });
        Ok(())
    }
}
