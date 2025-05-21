use lazy_static::lazy_static;
use std::{
    ffi::{c_void, CString},
    ptr::{copy_nonoverlapping, null, null_mut},
    sync::{Mutex, OnceLock},
};

use gl::{
    types::{GLboolean, GLchar, GLenum, GLfloat, GLint, GLsizei, GLsizeiptr, GLuint, GLvoid},
    BGRA, CLAMP_TO_EDGE, FRAGMENT_SHADER, LINEAR, PIXEL_PACK_BUFFER, POINTS, READ_ONLY, RGBA8,
    STREAM_READ, TEXTURE0, TEXTURE_2D, TEXTURE_MAG_FILTER, TEXTURE_MIN_FILTER, TEXTURE_WRAP_S,
    TEXTURE_WRAP_T, TRIANGLE_STRIP, UNSIGNED_BYTE, VERTEX_SHADER,
};

use windows::{
    core::PCSTR,
    Win32::Graphics::OpenGL::{
        glBindTexture, glDrawArrays, glGenTextures, glPointSize, glReadPixels, glTexParameteri,
        glTexSubImage2D, glViewport, wglGetProcAddress,
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
type GlBindVertexArrayFn = unsafe extern "system" fn(array: GLuint);
type GlUseProgramFn = unsafe extern "system" fn(program: GLuint);
type GlCreateVertexArraysFn = unsafe extern "system" fn(n: GLsizei, arrays: *mut GLuint);
type GlGenVertexArraysFn = unsafe extern "system" fn(n: GLsizei, arrays: *mut GLuint);
type GLUniform2FvFn =
    unsafe extern "system" fn(location: GLint, count: GLsizei, value: *const GLfloat);
type GLTexStorage2DFn = unsafe extern "system" fn(
    target: GLenum,
    levels: GLsizei,
    internalformat: GLenum,
    width: GLsizei,
    height: GLsizei,
);
type GLActiveTextureFn = unsafe extern "system" fn(texture: GLenum);
type GLBindTextureFn = unsafe extern "system" fn(target: GLenum, texture: GLuint);
type GLUniform1iFn = unsafe extern "system" fn(location: GLint, v0: GLint);
type GLGetUniformLocationFn =
    unsafe extern "system" fn(program: GLuint, name: *const GLchar) -> GLint;

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
static GL_CREATE_VERTEX_ARRAYS: OnceLock<GlCreateVertexArraysFn> = OnceLock::new();
static GL_GEN_VERTEX_ARRAYS: OnceLock<GlGenVertexArraysFn> = OnceLock::new();
static GL_UNIFORM_2FV: OnceLock<GLUniform2FvFn> = OnceLock::new();
static GL_TEX_STORAGE_2D: OnceLock<GLTexStorage2DFn> = OnceLock::new();
static GL_ACTIVE_TEXTURE: OnceLock<GLActiveTextureFn> = OnceLock::new();
static GL_BIND_TEXTURE: OnceLock<GLBindTextureFn> = OnceLock::new();
static GL_UNIFORM_1I: OnceLock<GLUniform1iFn> = OnceLock::new();
static GL_GET_UNIFORM_LOCATION: OnceLock<GLGetUniformLocationFn> = OnceLock::new();

static POINT_SHADER: OnceLock<GLuint> = OnceLock::new();
static VAO: OnceLock<GLuint> = OnceLock::new();
static OVERLAY_SHADER: OnceLock<GLuint> = OnceLock::new();
static TEXTURE: OnceLock<GLuint> = OnceLock::new();

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
            GlCreateVertexArraysFn,
            "glCreateVertexArrays"
        )
        && load!(
            GL_GEN_VERTEX_ARRAYS,
            GlGenVertexArraysFn,
            "glGenVertexArrays"
        )
        && load!(GL_UNIFORM_2FV, GLUniform2FvFn, "glUniform2fv")
        && load!(GL_TEX_STORAGE_2D, GLTexStorage2DFn, "glTexStorage2D")
        && load!(GL_ACTIVE_TEXTURE, GLActiveTextureFn, "glActiveTexture")
        && load!(GL_BIND_TEXTURE, GLBindTextureFn, "glBindTexture")
        && load!(GL_UNIFORM_1I, GLUniform1iFn, "glUniform1i")
        && load!(
            GL_GET_UNIFORM_LOCATION,
            GLGetUniformLocationFn,
            "glGetUniformLocation"
        )
}

lazy_static! {
    static ref PBO_DATA: Mutex<(Vec<u32>, i32, usize)> = Mutex::new((vec![0, 0], 0, 0)); //(PBOs, size, index)
}

pub fn read_frame(width: i32, height: i32, size: i32, dest: *mut u8) {
    if dest.is_null() {
        return;
    }

    let gl_bind_buffer = *GL_BIND_BUFFER.get().unwrap();
    let gl_map_buffer = *GL_MAP_BUFFER.get().unwrap();
    let gl_unmap_buffer = *GL_UNMAP_BUFFER.get().unwrap();

    let mut pbo_data = PBO_DATA.lock().unwrap();
    let (ref mut pbos, ref mut old_size, ref mut index) = *pbo_data;

    let row_stride = (width * 4) as usize;

    if pbos[0] == 0 {
        let gl_gen_buffers = *GL_GEN_BUFFERS.get().unwrap();
        let gl_buffer_data = *GL_BUFFER_DATA.get().unwrap();

        unsafe {
            gl_gen_buffers(2, pbos.as_mut_ptr());
            for &pbo in pbos.iter() {
                gl_bind_buffer(PIXEL_PACK_BUFFER, pbo);
                gl_buffer_data(PIXEL_PACK_BUFFER, size as isize, null(), STREAM_READ);
            }
        }

        *old_size = size;
    } else if *old_size != size {
        let gl_buffer_data = *GL_BUFFER_DATA.get().unwrap();
        unsafe {
            for &pbo in pbos.iter() {
                gl_bind_buffer(PIXEL_PACK_BUFFER, pbo);
                gl_buffer_data(PIXEL_PACK_BUFFER, size as isize, null(), STREAM_READ);
            }
        }

        *old_size = size;
    }

    let read_index = *index;
    let map_index = (read_index + 1) % 2;

    unsafe {
        // Read pixels into the read_index PBO
        gl_bind_buffer(PIXEL_PACK_BUFFER, pbos[read_index]);
        glReadPixels(0, 0, width, height, BGRA, UNSIGNED_BYTE, null_mut());

        // Map the previous frame's PBO to read its contents
        gl_bind_buffer(PIXEL_PACK_BUFFER, pbos[map_index]);
        let ptr = gl_map_buffer(PIXEL_PACK_BUFFER, READ_ONLY) as *const u8;

        if !ptr.is_null() {
            for row in 0..height as usize {
                let src_row = ptr.add(row * row_stride);
                let dest_row = dest.add((height as usize - 1 - row) * row_stride);
                copy_nonoverlapping(src_row, dest_row, row_stride);
            }

            gl_unmap_buffer(PIXEL_PACK_BUFFER); // Optional but recommended
        }

        *index = map_index; // Swap indices
    }
}

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

fn init_gl_resources_overlay() {
    let create_program = *GL_CREATE_PROGRAM.get().unwrap();
    let gen_vertex_arrays = *GL_GEN_VERTEX_ARRAYS.get().unwrap();
    let attach_shader = *GL_ATTACH_SHADER.get().unwrap();
    let link_program = *GL_LINK_PROGRAM.get().unwrap();
    let delete_shader = *GL_DELETE_SHADER.get().unwrap();

    const VS_SRC: &str = r#"
    #version 460 core
    const vec2 verts[4] = vec2[4](
        vec2(-1.0, -1.0),
        vec2( 1.0, -1.0),
        vec2(-1.0,  1.0),
        vec2( 1.0,  1.0)
    );
    out vec2 texCoord;
    void main() {
        texCoord = (verts[gl_VertexID].xy + 1.0) * 0.5;
        gl_Position = vec4(verts[gl_VertexID], 0.0, 1.0);
    }"#;

    const FS_SRC: &str = r#"
    #version 460 core
    in vec2 texCoord;
    out vec4 FragColor;
    uniform sampler2D screenTex;
    void main() {
        FragColor = texture(screenTex, vec2(texCoord.x, 1.0 - texCoord.y));
    }"#;

    let vs = compile_shader(VS_SRC, VERTEX_SHADER);
    let fs = compile_shader(FS_SRC, FRAGMENT_SHADER);

    unsafe {
        let program = create_program();
        attach_shader(program, vs);
        attach_shader(program, fs);
        link_program(program);

        delete_shader(vs);
        delete_shader(fs);
        OVERLAY_SHADER.set(program).unwrap();
    };

    if VAO.get().is_none() {
        let mut vao = 0;
        unsafe { gen_vertex_arrays(1, &mut vao) };
        VAO.set(vao).unwrap();
    }
}

fn init_gl_texture(width: i32, height: i32) {
    let gl_tex_storage_2d = *GL_TEX_STORAGE_2D.get().unwrap();
    let mut texture: GLuint = 0;
    unsafe {
        glGenTextures(1, &mut texture);
        glBindTexture(TEXTURE_2D, texture);
        glTexParameteri(TEXTURE_2D, TEXTURE_MIN_FILTER, LINEAR as i32);
        glTexParameteri(TEXTURE_2D, TEXTURE_MAG_FILTER, LINEAR as i32);
        glTexParameteri(TEXTURE_2D, TEXTURE_WRAP_S, CLAMP_TO_EDGE as i32);
        glTexParameteri(TEXTURE_2D, TEXTURE_WRAP_T, CLAMP_TO_EDGE as i32);
        gl_tex_storage_2d(TEXTURE_2D, 1, RGBA8, width, height);
    };
    TEXTURE.set(texture).unwrap();
}

pub fn draw_overlay(width: i32, height: i32, src: *const u8) {
    if OVERLAY_SHADER.get().is_none() {
        init_gl_resources_overlay();
    }

    if TEXTURE.get().is_none() {
        init_gl_texture(width, height);
    }

    let use_program = *GL_USE_PROGRAM.get().unwrap();
    let active_texture = *GL_ACTIVE_TEXTURE.get().unwrap();
    let bind_texture = *GL_BIND_TEXTURE.get().unwrap();
    let uniform_1i = *GL_UNIFORM_1I.get().unwrap();
    let get_uniform_location = *GL_GET_UNIFORM_LOCATION.get().unwrap();
    let bind_vertex_array = *GL_BIND_VERTEX_ARRAY.get().unwrap();

    let program = *OVERLAY_SHADER.get().unwrap();
    let texture = *TEXTURE.get().unwrap();
    let vao = *VAO.get().unwrap();

    // Upload new data every frame
    unsafe {
        glViewport(0, 0, width, height);
        glBindTexture(TEXTURE_2D, texture);
        glTexSubImage2D(
            TEXTURE_2D,
            0,
            0,
            0,
            width,
            height,
            BGRA,
            UNSIGNED_BYTE,
            src as *const c_void,
        );

        use_program(program);
        active_texture(TEXTURE0);
        bind_texture(TEXTURE_2D, texture);
        let name = CString::new("screenTex").unwrap();
        uniform_1i(get_uniform_location(program, name.as_ptr()), 0);

        bind_vertex_array(vao);
        glDrawArrays(TRIANGLE_STRIP, 0, 4);
    }
}

fn init_gl_resources() {
    let create_program = *GL_CREATE_PROGRAM.get().unwrap();
    let attach_shader = *GL_ATTACH_SHADER.get().unwrap();
    let link_program = *GL_LINK_PROGRAM.get().unwrap();
    let delete_shader = *GL_DELETE_SHADER.get().unwrap();
    let gen_vertex_arrays = *GL_GEN_VERTEX_ARRAYS.get().unwrap();

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
        let program = create_program();
        attach_shader(program, vs);
        attach_shader(program, fs);
        link_program(program);

        delete_shader(vs);
        delete_shader(fs);

        POINT_SHADER.set(program).unwrap();
    };

    if VAO.get().is_none() {
        let mut vao = 0;
        unsafe { gen_vertex_arrays(1, &mut vao) };
        VAO.set(vao).unwrap();
    }
}

pub fn draw_point(x: i32, y: i32, w: i32, h: i32) {
    if POINT_SHADER.get().is_none() {
        init_gl_resources();
    }

    let use_program = *GL_USE_PROGRAM.get().unwrap();
    let bind_vertex_array = *GL_BIND_VERTEX_ARRAY.get().unwrap();
    let uniform_2fv = *GL_UNIFORM_2FV.get().unwrap();

    let program = *POINT_SHADER.get().unwrap();
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
