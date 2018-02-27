#[macro_use]
extern crate shine_render_gl as render;
extern crate time;

use std::env;
use std::time::*;
use render::*;

#[derive(Copy, Clone, Debug)]
#[derive(VertexDeclaration)]
struct VxPos {
    position: Float32x3,
}

#[derive(Copy, Clone, Debug)]
#[derive(VertexDeclaration)]
struct VxColorTex {
    color: Float32x3,
    tex_coord: Float32x2,
}

#[derive(Copy, Clone, Debug)]
#[derive(ShaderDeclaration)]
#[vert_path = "fun.glsl"]
#[vert_src = "
    attribute vec3 vPosition;
    attribute vec3 vColor;
    attribute vec2 vTexCoord;
    uniform mat4 uTrsf;
    uniform vec3 uColor;
    varying vec3 color;
    varying vec2 txCoord;

    vec3 col_mod(vec3 c);

    void main()
    {
        color = col_mod(uColor + vColor);
        txCoord = vTexCoord.xy;
        gl_Position = uTrsf * vec4(vPosition, 1.0);
    }"]
#[frag_src = "
    varying vec3 color;
    varying vec2 txCoord;
    uniform sampler2D uTex;
    void main()
    {
        float intensity = texture2D( uTex, txCoord ).r;
        vec3 col =  color * intensity;
        gl_FragColor = vec4(col, 1.0);
    }"]
struct ShSimple {}

struct SimpleView {
    id: u8,
    t: f32,
    vb1: lowlevel::GLVertexBuffer,
    vb2: lowlevel::GLVertexBuffer,
    ib: lowlevel::GLIndexBuffer,
    sh: lowlevel::GLShaderProgram,
    tx: lowlevel::GLTexture,
}

unsafe impl Send for SimpleView {}

impl SimpleView {
    fn new(id: u8) -> SimpleView {
        SimpleView {
            id: id,
            t: 0.0,
            vb1: lowlevel::GLVertexBuffer::new(),
            vb2: lowlevel::GLVertexBuffer::new(),
            ib: lowlevel::GLIndexBuffer::new(),
            sh: lowlevel::GLShaderProgram::new(),
            tx: lowlevel::GLTexture::new(),
        }
    }

    fn on_surface_ready(&mut self, win: &mut GLWindow) {
        println!("surface ready");
        use lowlevel::*;
        let ll = win.backend().ll_mut();

        {
            let pos = [
                VxPos { position: (1., 0., 0.).into() },
                VxPos { position: (1., 1., 0.).into() },
                VxPos { position: (0., 1., 0.).into() },
                VxPos { position: (0., 0., 0.).into() },
            ];

            let VertexData::Transient(slice) = pos.to_data();
            self.vb1.upload_data(ll, VxPos::get_attribute_layout(), slice);
        }

        {
            let color_tex = [
                VxColorTex { color: (1., 0., 0.).into(), tex_coord: (1., 0.).into() },
                VxColorTex { color: (1., 1., 0.).into(), tex_coord: (1., 1.).into() },
                VxColorTex { color: (0., 1., 0.).into(), tex_coord: (0., 1.).into() },
                VxColorTex { color: (0., 0., 0.).into(), tex_coord: (0., 0.).into() }
            ];

            let VertexData::Transient(slice) = color_tex.to_data();
            self.vb2.upload_data(ll, VxColorTex::get_attribute_layout(), slice);
        }

        {
            let indices = [0u8, 1, 2, 0, 2, 3];

            let IndexData::Transient(slice) = indices.to_data();
            self.ib.upload_data(ll, IndexBinding::glenum_from_index_type(<u8 as IndexType>::get_layout()), slice);
        }

        {
            let img = include_bytes!("img.raw");
            let width = 1024;
            let height = 768;
            let format = PixelFormat::Rgb8;
            self.tx.upload_data(ll, gl::TEXTURE_2D, width, height, TextureBinding::glenum_from_pixel_format(format), img);
        }

        {
            self.sh.create_program(ll, ShSimple::source_iter());
            self.sh.parse_parameters(ll, ShSimpleParameters::get_index_by_name);
        }
    }

    fn on_surface_lost(&mut self, win: &mut GLWindow) {
        println!("surface lost");
        let ll = win.backend().ll_mut();
        self.vb1.release(ll);
        self.vb2.release(ll);
        self.ib.release(ll);
        self.tx.release(ll);
        self.sh.release(ll);
    }

    fn on_surface_changed(&mut self, win: &mut GLWindow) {
        println!("surface changed");
        //emulate full surface lost on window resize
        self.on_surface_lost(win);
        self.on_surface_ready(win);
    }

    fn on_tick(&mut self, win: &mut GLWindow) {
        use std::f32;
        self.t += 0.005f32;
        if self.t > 2. * f32::consts::PI {
            self.t = 0f32;
        }

        if win.is_ready_to_render() {
            self.on_render(win);
            win.swap_buffers().unwrap();
        }
    }

    fn on_render(&mut self, win: &mut GLWindow) {
        use render::lowlevel::*;
        let ll = win.backend().ll_mut();

        let id = self.id as f32;

        ll.init_view(Some(Viewport::Proportional(0.5, 0.5, 0.25, 0.25)),
                     Some(Float32x4(0.0, 0.0, id, 1.0)),
                     Some(1.));

        let st = self.t.sin();
        let ct = self.t.cos();
        let trsf = Float32x16::from(
            [st, -ct, 0.0, 0.0,
                ct, st, 0.0, 0.0,
                0.0, 0.0, 1.0, 0.0,
                0.0, 0.0, 0.0, 1.0]);
        let col = Float32x3::from([id / 3., self.t / 6.28, 0.5]);

        let vb1 = &mut self.vb1;
        let vb2 = &mut self.vb2;
        let ib = &mut self.ib;
        let tx = &mut self.tx;
        let sh = &mut self.sh;
        if sh.bind(ll) {
            if let Some(locations) = ll.program_binding.get_parameters() {
                let locations = &mut *locations.borrow_mut();

                locations[0].set_attribute(ll, &vb2, VxColorTex::COLOR);
                locations[1].set_attribute(ll, &vb2, VxColorTex::TEXCOORD);
                locations[2].set_attribute(ll, &vb1, VxPos::POSITION);

                locations[3].set_index(ll, &ib);

                locations[4].set_f32x3(ll, &col);
                locations[5].set_f32x16(ll, &trsf);
                locations[6].set_texture_2d(ll, &tx);
            }

            ll.draw(gl::TRIANGLES, 0, 6);
        }
    }

    fn on_key(&mut self, win: &mut GLWindow, virtual_key: Option<VirtualKeyCode>, is_down: bool) {
        match virtual_key {
            Some(VirtualKeyCode::Escape) if !is_down => { win.close(); }
            _ => {}
        }
    }
}

#[test]
pub fn simple_lowlevel() {
    assert!(env::var("RUST_TEST_THREADS").unwrap_or("0".to_string()) == "1", "This test shall run in single threaded test environment: RUST_TEST_THREADS=1");

    let engine = render::PlatformEngine::new().expect("Could not initialize render engine");

    let render_timeout = Duration::from_millis(20);
    let window = render::PlatformWindowSettings::default()
        .title("main")
        .size((1024, 1024))
        .fb_depth_bits(16, 8)
        .fb_vsync(true)
        .build(&engine,
               render::DispatchTimeout::Time(render_timeout),
               SimpleView::new(0),
               |window, view, cmd| {
                   match cmd {
                       &WindowCommand::SurfaceReady => view.on_surface_ready(window),
                       &WindowCommand::SurfaceLost => view.on_surface_lost(window),
                       &WindowCommand::SurfaceChanged => view.on_surface_changed(window),
                       &WindowCommand::KeyboardUp(_scan_code, virtual_key) => view.on_key(window, virtual_key, false),
                       &WindowCommand::KeyboardDown(_scan_code, virtual_key) => view.on_key(window, virtual_key, true),
                       &WindowCommand::Tick => view.on_tick(window),
                       _ => {}
                   }
               }).expect("Could not initialize main window");

    let mut sub_window1 = render::PlatformWindowSettings::default()
        .title("sub")
        .size((256, 256))
        .fb_depth_bits(16, 8)
        .fb_vsync(false)
        .build(&engine,
               render::DispatchTimeout::Time(render_timeout),
               SimpleView::new(1),
               |window, view, cmd| {
                   match cmd {
                       &WindowCommand::SurfaceReady => view.on_surface_ready(window),
                       &WindowCommand::SurfaceLost => view.on_surface_lost(window),
                       &WindowCommand::SurfaceChanged => view.on_surface_changed(window),
                       &WindowCommand::KeyboardUp(_scan_code, virtual_key) => view.on_key(window, virtual_key, false),
                       &WindowCommand::KeyboardDown(_scan_code, virtual_key) => view.on_key(window, virtual_key, true),
                       &WindowCommand::Tick => view.on_tick(window),
                       _ => {}
                   }
               }).expect("Could not initialize sub window");

    let mut sub_window2 = Some(render::PlatformWindowSettings::default()
        .title("sub2")
        .size((256, 256))
        .fb_depth_bits(16, 8)
        .fb_vsync(false)
        .build(&engine,
               render::DispatchTimeout::Time(render_timeout),
               SimpleView::new(2),
               |window, view, cmd| {
                   match cmd {
                       &WindowCommand::SurfaceReady => view.on_surface_ready(window),
                       &WindowCommand::SurfaceLost => view.on_surface_lost(window),
                       &WindowCommand::SurfaceChanged => view.on_surface_changed(window),
                       &WindowCommand::KeyboardUp(_scan_code, virtual_key) => view.on_key(window, virtual_key, false),
                       &WindowCommand::KeyboardDown(_scan_code, virtual_key) => view.on_key(window, virtual_key, true),
                       &WindowCommand::Tick => view.on_tick(window),
                       _ => {}
                   }
               }).expect("Could not initialize sub window"));


    let timeout = Duration::from_millis(100);
    let test_time = 5.;

    let mut start = time::precise_time_s();
    let mut test1 = false;
    let mut test2 = false;
    let mut test3 = false;
    while engine.dispatch_event(render::DispatchTimeout::Time(timeout)) {
        let now = time::precise_time_s();

        if now - start > test_time && !test1 {
            test1 = true;
            start = now;
            println!("test explicit close");
            sub_window1.close();
        }

        if now - start > test_time && !test2 {
            test2 = true;
            start = now;
            println!("test explicit drop");
            if let Some(win) = sub_window2.take() {
                drop(win);
            }
        }

        if now - start > test_time && !test3 {
            test3 = true;
            start = now;
            println!("press esc emulation");
            window.send_command(WindowCommand::KeyboardUp(0, Some(VirtualKeyCode::Escape)));
        }
    }

    drop(window);
    drop(sub_window1);
    drop(sub_window2);
}
