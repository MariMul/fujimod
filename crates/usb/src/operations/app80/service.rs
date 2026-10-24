//! Bind the selected firmware's codecs to its declared ff80 mailbox.
use super::*;
use crate::app80::catalog::{Catalog, Command, CommandError, CommandReply, PreparedCommand};
use fujimod_catalog_service::{Definition, ServiceModes};

#[derive(Clone, Debug)]
pub struct Service {
    catalog: Catalog,
    mailbox: Mailbox,
}
impl Service {
    pub fn new(definition: Definition) -> Result<Self> {
        let location = definition.mailbox();
        let mailbox = Mailbox::new(
            location.page(),
            location.offset(),
            location.configuration_limit(),
        )?;
        Ok(Self {
            catalog: Catalog::new(definition),
            mailbox,
        })
    }
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }
    pub fn prepare(
        &self,
        modes: ServiceModes,
        command: Command,
    ) -> std::result::Result<Invocation, CommandError> {
        Ok(Invocation {
            command: self.catalog.prepare(modes, command)?,
            mailbox: self.mailbox,
        })
    }
    pub fn prepare_code(
        &self,
        code: crate::app80::CommandCode,
        modes: ServiceModes,
        command: Command,
    ) -> std::result::Result<Invocation, CommandError> {
        Ok(Invocation {
            command: self.catalog.prepare_code(code, modes, command)?,
            mailbox: self.mailbox,
        })
    }
    pub fn prepare_research(
        &self,
        code: crate::app80::CommandCode,
        modes: ServiceModes,
        arguments: &[u8],
    ) -> std::result::Result<Invocation, CommandError> {
        Ok(Invocation {
            command: self.catalog.prepare_research(code, modes, arguments)?,
            mailbox: self.mailbox,
        })
    }
}
/// Its request and carrier originate from one firmware definition. Preparing an
/// invocation does no I/O and never changes service modes on the camera.
#[derive(Clone, Debug)]
pub struct Invocation {
    command: PreparedCommand,
    mailbox: Mailbox,
}
impl Invocation {
    pub fn command(&self) -> &PreparedCommand {
        &self.command
    }
    pub fn configuration_limit(&self) -> NonZeroU32 {
        self.mailbox.limit
    }
    /// Own the ff80 stage/publish/read exchange and validate the nested reply.
    pub async fn execute(&self, session: &mut Session) -> Result<CommandReply> {
        let reply = self
            .mailbox
            .exchange(session, self.command.request())
            .await?;
        match self.command.decode(reply) {
            Ok(reply) => Ok(reply),
            Err(error) => {
                session.invalidate_completion();
                Err(error)
            }
        }
    }
}
