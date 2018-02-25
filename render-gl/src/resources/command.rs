use std::mem;
use std::ptr;
use std::raw;
use std::ops::{Deref, DerefMut};
use store::fjsqueue::*;
use resources::*;
use libconfig::*;


#[derive(Copy, Clone, Debug)]
pub struct CommandOrder(pub u8, pub u32);


/// Commands using dynamic dispatch
pub trait DynCommand: 'static {
    fn process(&mut self, context: &mut GLCommandProcessContext);
}

impl<T: DynCommand> From<T> for Command {
    #[inline(always)]
    fn from(value: T) -> Command {
        if mem::size_of::<T>() > MINICOMMANDBOX_SIZE {
            Command::BoxedCommand(Box::new(value))
        } else {
            Command::MiniboxedCommand(MiniCommandBox::new(value))
        }
    }
}


/// Helper to omit some heap allocations
pub struct MiniCommandBox {
    data: [u8; MINICOMMANDBOX_SIZE],
    vtable: *mut (),
}

#[allow(dead_code)]
impl MiniCommandBox {
    fn new<T: DynCommand>(t: T) -> MiniCommandBox {
        assert!(mem::size_of::<T>() <= MINICOMMANDBOX_SIZE, format!("increase MINICOMMANDBOX_SIZE to at least {}", mem::size_of::<T>()));

        unsafe {
            let mut bx = MiniCommandBox {
                data: mem::uninitialized(),
                vtable: {
                    let obj: &DynCommand = &t;
                    let obj: raw::TraitObject = mem::transmute(obj);
                    obj.vtable
                },
            };

            ptr::write(&mut bx.data as *mut _ as *mut u8 as *mut T, t);
            bx
        }
    }

    fn as_ref(&self) -> &DynCommand {
        unsafe {
            mem::transmute(raw::TraitObject {
                data: mem::transmute(self.data.as_ptr()),
                vtable: self.vtable,
            })
        }
    }

    fn as_mut(&mut self) -> &mut DynCommand {
        unsafe {
            mem::transmute(raw::TraitObject {
                data: mem::transmute(self.data.as_ptr()),
                vtable: self.vtable,
            })
        }
    }
}

impl Deref for MiniCommandBox {
    type Target = DynCommand;

    fn deref(&self) -> &DynCommand {
        self.as_ref()
    }
}

impl DerefMut for MiniCommandBox {
    fn deref_mut(&mut self) -> &mut DynCommand {
        self.as_mut()
    }
}
/*
impl Drop for MiniCommandBox {
    fn drop(&mut self) {
        let obj = raw::TraitObject {
            data: mem::transmute(self.data.as_ptr()),
            vtable: self.vtable,
        };
        (obj.vtable.destructor)(self.data.as_ptr());
    }
}*/


/// Enum for render commands.
pub enum Command {
    //VertexCreate(vertexbuffer::CreateCommand),
    Clear(backend::ClearCommand),
    VertexRelease(vertexbuffer::ReleaseCommand),
    IndexCreate(indexbuffer::CreateCommand),
    IndexRelease(indexbuffer::ReleaseCommand),
    Texture2DCreate(texture2d::CreateCommand),
    Texture2DRelease(texture2d::ReleaseCommand),
    //ShaderProgramCreate(shaderprogram::CreateCommand),
    ShaderProgramRelease(shaderprogram::ReleaseCommand),

    MiniboxedCommand(MiniCommandBox),
    BoxedCommand(Box<DynCommand>),
}

impl Command {
    pub fn process(&mut self, context: &mut GLCommandProcessContext) {
        use Command::*;
        match self {
            &mut Clear(ref mut cmd) => cmd.process(context),
            //&VertexCreate(ref mut cmd) => cmd.process(ll, flush),
            &mut VertexRelease(ref mut cmd) => cmd.process(context),
            &mut IndexCreate(ref mut cmd) => cmd.process(context),
            &mut IndexRelease(ref mut cmd) => cmd.process(context),
            &mut Texture2DCreate(ref mut cmd) => cmd.process(context),
            &mut Texture2DRelease(ref mut cmd) => cmd.process(context),
            //&ShaderProgramCreate(ref mut cmd) => cmd.process(context),
            &mut ShaderProgramRelease(ref mut cmd) => cmd.process(context),
            &mut BoxedCommand(ref mut cmd) => cmd.process(context),
            &mut MiniboxedCommand(ref mut cmd) => cmd.process(context),
        }
    }
}


pub type CommandStore = FJSQueue<CommandOrder, Command>;
pub type CommandProduceGuard<'a> = ProduceGuard<'a, CommandOrder, Command>;
