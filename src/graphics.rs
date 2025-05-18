use lazy_static::lazy_static;
use std::{
    ffi::{c_void, CString},
    ptr::{copy_nonoverlapping, null, null_mut},
    sync::{Mutex, OnceLock},
};

use gl::{
    types::{GLboolean, GLchar, GLenum, GLfloat, GLint, GLsizei, GLsizeiptr, GLuint, GLvoid},
    BGRA, FRAGMENT_SHADER, PIXEL_PACK_BUFFER, POINTS, READ_ONLY, STREAM_READ, UNSIGNED_BYTE,
    VERTEX_SHADER,
};

use windows::{
    core::PCSTR,
    Win32::Graphics::OpenGL::{glDrawArrays, glPointSize, glReadPixels, wglGetProcAddress},
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
type GlBindVertexArrayFn = unsafe extern "system" fn(array: GLuint);

type GlUseProgramFn = unsafe extern "system" fn(program: GLuint);

type GlCreateVertexArrays = unsafe extern "system" fn(n: GLsizei, arrays: *mut GLuint);

type GLUniform2Fv =
    unsafe extern "system" fn(location: GLint, count: GLsizei, value: *const GLfloat);

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
static GL_BIND_VERTEX_ARRAY: OnceLock<GlBindVertexArrayFn> = OnceLock::new();
static GL_USE_PROGRAM: OnceLock<GlUseProgramFn> = OnceLock::new();
static GL_CREATE_VERTEX_ARRAYS: OnceLock<GlCreateVertexArrays> = OnceLock::new();
static GL_UNIFORM_2FV: OnceLock<GLUniform2Fv> = OnceLock::new();

static SHADER_PROGRAM: OnceLock<GLuint> = OnceLock::new();
static VAO: OnceLock<GLuint> = OnceLock::new();

pub fn restore_state(prev_program: i32, prev_vao: i32) {
    let use_program = *GL_USE_PROGRAM.get().unwrap();
    let bind_vertex_array = *GL_BIND_VERTEX_ARRAY.get().unwrap();

    unsafe {
        use_program(prev_program as GLuint);
        bind_vertex_array(prev_vao as GLuint);
    }
}

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
            GL_BIND_VERTEX_ARRAY,
            GlBindVertexArrayFn,
            "glBindVertexArray"
        )
        && load!(GL_USE_PROGRAM, GlUseProgramFn, "glUseProgram")
        && load!(
            GL_CREATE_VERTEX_ARRAYS,
            GlCreateVertexArrays,
            "glCreateVertexArrays"
        )
        && load!(GL_UNIFORM_2FV, GLUniform2Fv, "glUniform2fv")
}

lazy_static! {
    static ref PBO_DATA: Mutex<(u32, i32)> = Mutex::new((0, 0));
}

pub fn read_frame(width: i32, height: i32, size: i32, dest: *mut u8) {
    if dest.is_null() {
        return;
    }

    let gl_bind_buffer = *GL_BIND_BUFFER.get().unwrap();

    let mut pbo_data = PBO_DATA.lock().unwrap();

    let (pbo, old_size) = *pbo_data;
    let mut recalculated = false;

    if pbo == 0 {
        recalculated = true;
        let gl_gen_buffers = *GL_GEN_BUFFERS.get().unwrap();
        let gl_buffer_data = *GL_BUFFER_DATA.get().unwrap();
        let mut new_pbo = 0;
        unsafe {
            gl_gen_buffers(1, &mut new_pbo);
            gl_bind_buffer(PIXEL_PACK_BUFFER, new_pbo);
            *pbo_data = (new_pbo, size);
            gl_buffer_data(PIXEL_PACK_BUFFER, size as isize, null(), STREAM_READ);
        }
    } else if size != old_size {
        //Resize PBO if dimensions changed
        recalculated = true;
        let gl_buffer_data = *GL_BUFFER_DATA.get().unwrap();
        unsafe {
            gl_bind_buffer(PIXEL_PACK_BUFFER, pbo);
            gl_buffer_data(PIXEL_PACK_BUFFER, size as isize, null(), STREAM_READ);
        }
        *pbo_data = (pbo, size);
    }

    unsafe {
        if !recalculated {
            gl_bind_buffer(PIXEL_PACK_BUFFER, pbo);
        }
        glReadPixels(0, 0, width, height, BGRA, UNSIGNED_BYTE, null_mut());

        let gl_map_buffer = *GL_MAP_BUFFER.get().unwrap();
        let pbo_ptr = gl_map_buffer(PIXEL_PACK_BUFFER, READ_ONLY) as *const u8;

        if !pbo_ptr.is_null() {
            let row_stride = (width * 4) as usize;

            for row in 0..height as usize {
                let src_row_start = pbo_ptr.add(row * row_stride);
                let dest_row_start = dest.add((height as usize - 1 - row) * row_stride);
                copy_nonoverlapping(src_row_start, dest_row_start, row_stride);
            }

            let gl_unmap_buffer = *GL_UNMAP_BUFFER.get().unwrap();
            gl_unmap_buffer(PIXEL_PACK_BUFFER);
        }
    }
}

/* fn print_gl_errors(name: &str) {
    loop {
        let error = unsafe { glGetError() };
        if error == GL_NO_ERROR {
            break;
        }
        println!("{} error: {:x}\r\n", name, error);
    }
}
 */

fn compile_shader(source: &str, shader_type: GLenum) -> GLuint {
    let gl_create_shader = *GL_CREATE_SHADER.get().unwrap();
    let gl_shader_source = *GL_SHADER_SOURCE.get().unwrap();
    let gl_compile_shader = *GL_COMPILE_SHADER.get().unwrap();

    let c_str = CString::new(source.as_bytes()).unwrap();

    unsafe {
        let shader = gl_create_shader(shader_type);
        gl_shader_source(shader, 1, &c_str.as_ptr(), null());
        gl_compile_shader(shader);
        shader
    }
}

fn init_gl_resources() {
    let gl_create_program = *GL_CREATE_PROGRAM.get().unwrap();
    let gl_attach_shader = *GL_ATTACH_SHADER.get().unwrap();
    let gl_link_program = *GL_LINK_PROGRAM.get().unwrap();
    let gl_delete_shader = *GL_DELETE_SHADER.get().unwrap();
    let gl_create_vertex_arrays = *GL_CREATE_VERTEX_ARRAYS.get().unwrap();

    const VS_SRC: &str = r#"
    #version 460 core
    layout(location = 0) uniform vec2 pointPos;
    void main() {
        gl_Position = vec4(pointPos, 0.0, 1.0);
    }"#;

    const FS_SRC: &str = r#"
    #version 460 core
    out vec4 FragColor;
    void main() {
        vec2 coord = gl_PointCoord * 2.0 - 1.0;
        float dist = length(coord);
        if (dist > 1.0) discard;
        FragColor = vec4(1.0, 0.0, 0.0, 1.0);
    }"#;

    let vs = compile_shader(VS_SRC, VERTEX_SHADER);
    let fs = compile_shader(FS_SRC, FRAGMENT_SHADER);

    unsafe {
        let program = gl_create_program();
        gl_attach_shader(program, vs);
        gl_attach_shader(program, fs);
        gl_link_program(program);

        gl_delete_shader(vs);
        gl_delete_shader(fs);

        let mut vao = 0;
        gl_create_vertex_arrays(1, &mut vao);

        SHADER_PROGRAM.set(program).unwrap();
        VAO.set(vao).unwrap();
    };
}

pub fn draw_point(x: i32, y: i32, w: i32, h: i32) {
    if SHADER_PROGRAM.get().is_none() {
        init_gl_resources();
    }

    let use_program = *GL_USE_PROGRAM.get().unwrap();
    let bind_vertex_array = *GL_BIND_VERTEX_ARRAY.get().unwrap();
    let uniform_2fv = *GL_UNIFORM_2FV.get().unwrap();

    let program = *SHADER_PROGRAM.get().unwrap();
    let vao = *VAO.get().unwrap();

    let x_ndc = (x as f32 / w as f32) * 2.0 - 1.0;
    let y_ndc = 1.0 - (y as f32 / h as f32) * 2.0;
    let pos = [x_ndc, y_ndc];

    unsafe {
        use_program(program);
        bind_vertex_array(vao);
        glPointSize(6.0);
        uniform_2fv(0, 1, pos.as_ptr());
        glDrawArrays(POINTS, 0, 1);
    };
}
