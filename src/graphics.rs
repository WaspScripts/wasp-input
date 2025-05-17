use std::{
    collections::HashMap,
    ffi::{c_void, CString},
    ptr::{null, null_mut},
    sync::{
        atomic::{AtomicI32, AtomicUsize, Ordering},
        OnceLock,
    },
};

use crate::Mutex;
use gl::{
    types::{
        GLboolean, GLchar, GLenum, GLfloat, GLint, GLsizei, GLsizeiptr, GLubyte, GLuint, GLvoid,
    },
    ARRAY_BUFFER, BGRA, BLEND, DYNAMIC_DRAW, FRAGMENT_SHADER, NICEST, ONE_MINUS_SRC_ALPHA,
    PIXEL_PACK_BUFFER, POINTS, READ_ONLY, STREAM_READ, TEXTURE_2D, TEXTURE_RECTANGLE,
    UNPACK_ROW_LENGTH, UNSIGNED_BYTE, VERTEX_SHADER,
};
use lazy_static::lazy_static;
use windows::{
    core::PCSTR,
    Win32::Graphics::{
        Gdi::HDC,
        OpenGL::{
            glBegin, glBlendFunc, glColor3f, glColor4ub, glDisable, glDrawArrays, glEnable, glEnd,
            glFlush, glGetError, glGetFloatv, glHint, glIsEnabled, glLoadIdentity, glMatrixMode,
            glOrtho, glPixelStorei, glPointSize, glPopAttrib, glPopMatrix, glPushAttrib,
            glPushMatrix, glRasterPos2f, glReadPixels, glVertex2i, glVertex3f, glViewport,
            wglCreateContext, wglGetProcAddress, wglMakeCurrent, GetPixelFormat,
            GL_ALL_ATTRIB_BITS, GL_DEPTH_TEST, GL_FALSE, GL_FASTEST, GL_FLOAT, GL_MODELVIEW,
            GL_NO_ERROR, GL_POINTS, GL_POINT_SIZE, GL_POINT_SMOOTH, GL_POINT_SMOOTH_HINT,
            GL_PROJECTION, GL_SRC_ALPHA, HGLRC,
        },
    },
};

type GlGenBuffersFn = unsafe extern "system" fn(n: GLsizei, buffers: *mut GLuint);
type GlDeleteBuffersFn = unsafe extern "system" fn(n: GLsizei, buffers: *const GLuint);
type GlBindBufferFn = unsafe extern "system" fn(target: GLenum, buffer: GLuint);
type GlBufferDataFn =
    unsafe extern "system" fn(target: GLenum, size: GLsizeiptr, data: *const GLvoid, usage: GLenum);
type GlMapBufferFn = unsafe extern "system" fn(target: GLenum, access: GLenum) -> *mut c_void;
type GlUnmapBufferFn = unsafe extern "system" fn(target: GLenum) -> GLboolean;
type GlCreateShaderFn = unsafe extern "system" fn(shader_type: GLenum) -> GLuint;
type GlShaderSourceFn = unsafe extern "system" fn(
    shader: GLuint,
    count: GLsizei,
    string: *const *const GLchar,
    length: *const GLint,
);

type GlCompileShaderFn = unsafe extern "system" fn(shader: GLuint);
type GlCreateProgramFn = unsafe extern "system" fn() -> GLuint;
type GlAttachShaderFn = unsafe extern "system" fn(program: GLuint, shader: GLuint);
type GlLinkProgramFn = unsafe extern "system" fn(program: GLuint);
type GlDeleteShaderFn = unsafe extern "system" fn(shader: GLuint);
type GlGenVertexArraysFn = unsafe extern "system" fn(n: GLsizei, arrays: *mut GLuint);
type GlBindVertexArrayFn = unsafe extern "system" fn(array: GLuint);
type GlVertexAttribPointerFn = unsafe extern "system" fn(
    index: GLuint,
    size: GLint,
    type_: GLenum,
    normalized: GLboolean,
    stride: GLsizei,
    pointer: *const c_void,
);
type GlUseProgramFn = unsafe extern "system" fn(program: GLuint);
type GlEnableVertexAttribArrayFn = unsafe extern "system" fn(index: GLuint);

static GL_GEN_BUFFERS: OnceLock<GlGenBuffersFn> = OnceLock::new();
static GL_DELETE_BUFFERS: OnceLock<GlDeleteBuffersFn> = OnceLock::new();
static GL_BIND_BUFFER: OnceLock<GlBindBufferFn> = OnceLock::new();
static GL_BUFFER_DATA: OnceLock<GlBufferDataFn> = OnceLock::new();
static GL_MAP_BUFFER: OnceLock<GlMapBufferFn> = OnceLock::new();
static GL_UNMAP_BUFFER: OnceLock<GlUnmapBufferFn> = OnceLock::new();
static GL_CREATE_SHADER: OnceLock<GlCreateShaderFn> = OnceLock::new();
static GL_SHADER_SOURCE: OnceLock<GlShaderSourceFn> = OnceLock::new();
static GL_COMPILE_SHADER: OnceLock<GlCompileShaderFn> = OnceLock::new();
static GL_CREATE_PROGRAM: OnceLock<GlCreateProgramFn> = OnceLock::new();
static GL_ATTACH_SHADER: OnceLock<GlAttachShaderFn> = OnceLock::new();
static GL_LINK_PROGRAM: OnceLock<GlLinkProgramFn> = OnceLock::new();
static GL_DELETE_SHADER: OnceLock<GlDeleteShaderFn> = OnceLock::new();
static GL_GEN_VERTEX_ARRAYS: OnceLock<GlGenVertexArraysFn> = OnceLock::new();
static GL_BIND_VERTEX_ARRAY: OnceLock<GlBindVertexArrayFn> = OnceLock::new();
static GL_VERTEX_ATTRIB_POINTER: OnceLock<GlVertexAttribPointerFn> = OnceLock::new();
static GL_USE_PROGRAM: OnceLock<GlUseProgramFn> = OnceLock::new();
static GL_ENABLE_VERTEX_ATTRIB_ARRAY: OnceLock<GlEnableVertexAttribArrayFn> = OnceLock::new();

pub unsafe fn load_opengl_extensions() -> bool {
    let load_fn = |name: &str| -> *const c_void {
        let cname = CString::new(name).unwrap();
        let pcstr = PCSTR(cname.as_ptr() as *const u8);
        match wglGetProcAddress(pcstr) {
            Some(f) => f as *const c_void,

            None => null(),
        }
    };

    macro_rules! load {
        ($sym:ident, $type:ty, $name:literal) => {{
            if $sym.get().is_some() {
                true
            } else {
                let ptr = load_fn($name);
                if ptr.is_null() {
                    return false;
                }
                $sym.set(std::mem::transmute::<*const c_void, $type>(ptr))
                    .is_ok()
            }
        }};
    }

    load!(GL_GEN_BUFFERS, GlGenBuffersFn, "glGenBuffers")
        && load!(GL_DELETE_BUFFERS, GlDeleteBuffersFn, "glDeleteBuffers")
        && load!(GL_BIND_BUFFER, GlBindBufferFn, "glBindBuffer")
        && load!(GL_BUFFER_DATA, GlBufferDataFn, "glBufferData")
        && load!(GL_MAP_BUFFER, GlMapBufferFn, "glMapBuffer")
        && load!(GL_UNMAP_BUFFER, GlUnmapBufferFn, "glUnmapBuffer")
        && load!(GL_CREATE_SHADER, GlCreateShaderFn, "glCreateShader")
        && load!(GL_SHADER_SOURCE, GlShaderSourceFn, "glShaderSource")
        && load!(GL_COMPILE_SHADER, GlCompileShaderFn, "glCompileShader")
        && load!(GL_CREATE_PROGRAM, GlCreateProgramFn, "glCreateProgram")
        && load!(GL_ATTACH_SHADER, GlAttachShaderFn, "glAttachShader")
        && load!(GL_LINK_PROGRAM, GlLinkProgramFn, "glLinkProgram")
        && load!(GL_DELETE_SHADER, GlDeleteShaderFn, "glDeleteShader")
        && load!(
            GL_GEN_VERTEX_ARRAYS,
            GlGenVertexArraysFn,
            "glGenVertexArrays"
        )
        && load!(
            GL_BIND_VERTEX_ARRAY,
            GlBindVertexArrayFn,
            "glBindVertexArray"
        )
        && load!(
            GL_VERTEX_ATTRIB_POINTER,
            GlVertexAttribPointerFn,
            "glVertexAttribPointer"
        )
        && load!(GL_USE_PROGRAM, GlUseProgramFn, "glUseProgram")
        && load!(
            GL_ENABLE_VERTEX_ATTRIB_ARRAY,
            GlEnableVertexAttribArrayFn,
            "glEnableVertexAttribArray"
        )
}

#[derive(Copy, Clone)]
struct SafeHGLRC(HGLRC);

unsafe impl Send for SafeHGLRC {}
unsafe impl Sync for SafeHGLRC {}

lazy_static! {
    static ref CONTEXTS: Mutex<HashMap<i32, SafeHGLRC>> = Mutex::new(HashMap::new());
}

pub unsafe fn push_gl_context(hdc: HDC, width: i32, height: i32) {
    let pixelformat = GetPixelFormat(hdc);

    let mut contexts = CONTEXTS.lock().unwrap();
    if !contexts.contains_key(&pixelformat) {
        let ctx = wglCreateContext(hdc).expect("Failed to create OpenGL context");
        contexts.insert(pixelformat, SafeHGLRC(ctx));
    }

    let SafeHGLRC(ctx) = *contexts.get(&pixelformat).unwrap();
    let _ = wglMakeCurrent(hdc, ctx);

    glPushAttrib(GL_ALL_ATTRIB_BITS);

    glPushMatrix();
    glViewport(0, 0, width, height);
    glMatrixMode(GL_PROJECTION);
    glLoadIdentity();
    glOrtho(0.0, width as f64, 0.0, height as f64, -1.0, 1.0);
    glMatrixMode(GL_MODELVIEW);
    glLoadIdentity();
    glDisable(GL_DEPTH_TEST);
}

pub unsafe fn pop_gl_context(hdc: HDC, ctx: HGLRC) {
    glPopMatrix();
    glPopAttrib();
    let _ = wglMakeCurrent(hdc, ctx);
}

static WIDTH: AtomicI32 = AtomicI32::new(0);
static HEIGHT: AtomicI32 = AtomicI32::new(0);

pub unsafe fn generate_pixel_buffers(
    pbo: &mut [GLuint; 2],
    width: GLint,
    height: GLint,
    stride: GLint,
) {
    let w = WIDTH.load(Ordering::Relaxed);
    let h = HEIGHT.load(Ordering::Relaxed);

    if (w != width) || (h != height) {
        let gl_gen_buffers = *GL_GEN_BUFFERS.get().unwrap();
        let gl_buffer_data = *GL_BUFFER_DATA.get().unwrap();
        let gl_bind_buffer = *GL_BIND_BUFFER.get().unwrap();
        let gl_delete_buffers = *GL_DELETE_BUFFERS.get().unwrap();

        WIDTH.store(width, Ordering::Relaxed);
        HEIGHT.store(height, Ordering::Relaxed);

        if pbo[1] != 0 {
            gl_delete_buffers(2, pbo.as_ptr());
            pbo[0] = 0;
            pbo[1] = 0;
        }

        gl_gen_buffers(2, pbo.as_mut_ptr());

        let buffer_size = (width * height * stride) as GLsizeiptr;

        gl_bind_buffer(PIXEL_PACK_BUFFER, pbo[0]);
        gl_buffer_data(PIXEL_PACK_BUFFER, buffer_size, null(), STREAM_READ);
        gl_bind_buffer(PIXEL_PACK_BUFFER, 0);

        gl_bind_buffer(PIXEL_PACK_BUFFER, pbo[1]);
        gl_buffer_data(PIXEL_PACK_BUFFER, buffer_size, null(), STREAM_READ);
        gl_bind_buffer(PIXEL_PACK_BUFFER, 0);
    }
}

static INDEX: AtomicUsize = AtomicUsize::new(0);

pub fn flip_image_bytes(input: *const u8, output: *mut u8, width: i32, height: i32, bpp: u32) {
    let chunk = if bpp > 24 {
        (width as usize) * 4
    } else {
        (width as usize) * 3 + (width as usize) % 4
    };

    unsafe {
        let mut source = input.add(chunk * (height as usize - 1));
        let mut destination = output;

        while source != input {
            std::ptr::copy_nonoverlapping(source, destination, chunk);
            destination = destination.add(chunk);
            source = source.sub(chunk);
        }
    }
}

pub unsafe fn read_pixel_buffers(
    dest: *mut GLubyte,
    pbo: &mut [GLuint; 2],
    width: GLint,
    height: GLint,
) {
    let index_cell = INDEX.load(Ordering::Relaxed);
    let gl_bind_buffer = *GL_BIND_BUFFER.get().unwrap();
    let gl_unmap_buffer = *GL_UNMAP_BUFFER.get().unwrap();
    let gl_map_buffer = *GL_MAP_BUFFER.get().unwrap();

    // Swap indices
    let index = (index_cell + 1) % 2;
    let next_index = (index + 1) % 2;
    INDEX.store(index, Ordering::Relaxed);

    glPixelStorei(UNPACK_ROW_LENGTH, width);
    gl_bind_buffer(PIXEL_PACK_BUFFER, pbo[index]);
    glReadPixels(0, 0, width, height, BGRA, UNSIGNED_BYTE, null_mut());
    gl_bind_buffer(PIXEL_PACK_BUFFER, pbo[next_index]);

    let data = gl_map_buffer(PIXEL_PACK_BUFFER, READ_ONLY);

    if !data.is_null() {
        flip_image_bytes(data as *const u8, dest.cast(), width, height, 32);
        gl_unmap_buffer(PIXEL_PACK_BUFFER);
    } else {
        glReadPixels(0, 0, width, height, BGRA, UNSIGNED_BYTE, dest as *mut _);
    }

    gl_bind_buffer(PIXEL_PACK_BUFFER, 0);
    glPixelStorei(UNPACK_ROW_LENGTH, 0);
}

pub unsafe fn gl_draw_point(x: f32, y: f32) {
    glColor4ub(0xFF, 0x00, 0x00, 0xFF);
    let mut point_size = 0.0;
    let gl_blend = glIsEnabled(BLEND) != 0;
    let gl_texture_2d = glIsEnabled(TEXTURE_2D) != 0;
    let gl_rectangle = glIsEnabled(TEXTURE_RECTANGLE) != 0;
    let point_smooth = glIsEnabled(GL_POINT_SMOOTH) != 0;
    glGetFloatv(GL_POINT_SIZE, &mut point_size);

    // Set new state
    glEnable(BLEND);
    glBlendFunc(GL_SRC_ALPHA, ONE_MINUS_SRC_ALPHA);

    glDisable(TEXTURE_2D);
    glEnable(GL_POINT_SMOOTH);
    glHint(GL_POINT_SMOOTH_HINT, NICEST);

    glPushMatrix();
    glLoadIdentity();

    // Draw Point
    glRasterPos2f(x, y);
    glPointSize(4.0);
    glBegin(POINTS);
    glVertex3f(x, y, 0.0);
    glEnd();
    glFlush();

    // Restore state
    glPopMatrix();

    if !gl_blend {
        glDisable(BLEND);
    }

    if gl_texture_2d {
        glEnable(TEXTURE_2D);
    }

    if gl_rectangle {
        glEnable(TEXTURE_RECTANGLE);
    }

    if !point_smooth {
        glDisable(GL_POINT_SMOOTH);
    }

    glPointSize(point_size);
}

pub static SHADER_PROGRAM: OnceLock<u32> = OnceLock::new();

pub static VAO: OnceLock<u32> = OnceLock::new();

pub static VBO: OnceLock<u32> = OnceLock::new();

pub unsafe fn init_gl() {
    let gl_create_shader = *GL_CREATE_SHADER.get().unwrap();
    let gl_shader_source = *GL_SHADER_SOURCE.get().unwrap();
    let gl_comile_shader = *GL_COMPILE_SHADER.get().unwrap();
    let gl_create_program = *GL_CREATE_PROGRAM.get().unwrap();
    let gl_attach_shader = *GL_ATTACH_SHADER.get().unwrap();
    let gl_link_program = *GL_LINK_PROGRAM.get().unwrap();
    let gl_delete_shader = *GL_DELETE_SHADER.get().unwrap();
    let gl_gen_vertex_arrays = *GL_GEN_VERTEX_ARRAYS.get().unwrap();
    let gl_gen_buffers = *GL_GEN_BUFFERS.get().unwrap();
    let gl_bind_vertex_array = *GL_BIND_VERTEX_ARRAY.get().unwrap();
    let gl_bind_buffer = *GL_BIND_BUFFER.get().unwrap();
    let gl_vertex_attrib_pointer = *GL_VERTEX_ATTRIB_POINTER.get().unwrap();
    let gl_enable_vertex_attrib_array = *GL_ENABLE_VERTEX_ATTRIB_ARRAY.get().unwrap();

    let vertex_shader_src = b"#version 460 core\nlayout(location = 0) in vec2 aPos;\nvoid main() {\n gl_Position = vec4(aPos, 0.0, 1.0);\n}\0";
    let fragment_shader_src = b"#version 460 core\nout vec4 FragColor;\nvoid main() {\n FragColor = vec4(1.0, 0.0, 0.0, 1.0);\n}\0";

    let vertex_shader = gl_create_shader(VERTEX_SHADER);

    gl_shader_source(
        vertex_shader,
        1,
        [vertex_shader_src.as_ptr().cast()].as_ptr(),
        null(),
    );

    gl_comile_shader(vertex_shader);

    let fragment_shader = gl_create_shader(FRAGMENT_SHADER);

    gl_shader_source(
        fragment_shader,
        1,
        [fragment_shader_src.as_ptr().cast()].as_ptr(),
        std::ptr::null(),
    );

    gl_comile_shader(fragment_shader);

    let shader_program = gl_create_program();

    gl_attach_shader(shader_program, vertex_shader);
    gl_attach_shader(shader_program, fragment_shader);
    gl_link_program(shader_program);
    gl_delete_shader(vertex_shader);
    gl_delete_shader(fragment_shader);

    let mut vao = 0;
    let mut vbo = 0;

    gl_gen_vertex_arrays(1, &mut vao);
    gl_gen_buffers(1, &mut vbo);
    gl_bind_vertex_array(vao);
    gl_bind_buffer(ARRAY_BUFFER, vbo);

    gl_vertex_attrib_pointer(
        0,
        2,
        GL_FLOAT,
        GL_FALSE as u8,
        2 * size_of::<f32>() as i32,
        null(),
    );

    gl_enable_vertex_attrib_array(0);

    SHADER_PROGRAM.set(shader_program).ok();
    VAO.set(vao).ok();
    VBO.set(vbo).ok();
}

pub unsafe fn draw_pt(x: i32, y: i32, w: i32, h: i32) {
    if SHADER_PROGRAM.get().is_none() {
        init_gl();
    }

    let gl_use_program = *GL_USE_PROGRAM.get().unwrap();
    let gl_bind_vertex_array = *GL_BIND_VERTEX_ARRAY.get().unwrap();
    let gl_bind_buffer = *GL_BIND_BUFFER.get().unwrap();
    let gl_buffer_data = *GL_BUFFER_DATA.get().unwrap();

    let shader_program = *SHADER_PROGRAM.get().unwrap();
    let vao = *VAO.get().unwrap();
    let vbo = *VBO.get().unwrap();

    let gl_blend = glIsEnabled(BLEND) != 0;

    // Convert mouse pos to NDC
    let x_ndc = (x as f32 / w as f32) * 2.0 - 1.0;
    let y_ndc = 1.0 - (y as f32 / h as f32) * 2.0;
    let vertex = [x_ndc, y_ndc];

    // Draw the point

    glEnable(BLEND);
    glBlendFunc(GL_SRC_ALPHA, ONE_MINUS_SRC_ALPHA);

    gl_use_program(shader_program);
    gl_bind_vertex_array(vao);
    gl_bind_buffer(ARRAY_BUFFER, vbo);

    gl_buffer_data(
        ARRAY_BUFFER,
        size_of_val(&vertex) as isize,
        vertex.as_ptr().cast(),
        DYNAMIC_DRAW,
    );

    glPointSize(4.0);
    glDrawArrays(GL_POINTS, 0, 1);

    if !gl_blend {
        glDisable(BLEND);
    }
}
