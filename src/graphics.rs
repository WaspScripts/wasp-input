use gl::types::{GLenum, GLfloat};
use windows::Win32::Graphics::OpenGL::{
    glBegin, glBlendFunc, glDisable, glEnable, glEnd, glFlush, glGetFloatv, glHint, glIsEnabled,
    glLoadIdentity, glPointSize, glPopMatrix, glPushMatrix, glRasterPos2f, glVertex3f, GL_BLEND,
    GL_NICEST, GL_ONE_MINUS_SRC_ALPHA, GL_POINTS, GL_POINT_SIZE, GL_POINT_SMOOTH,
    GL_POINT_SMOOTH_HINT, GL_SRC_ALPHA, GL_TEXTURE_2D,
};

pub unsafe fn gl_draw_point(x: f32, y: f32, z: f32, radius: f32) {
    const GL_TEXTURE_RECTANGLE: GLenum = 0x84F5;

    // Backup current OpenGL states
    let mut point_size: GLfloat = 0.0;
    let gl_blend = glIsEnabled(GL_BLEND) != 0;
    let gl_texture_2d = glIsEnabled(GL_TEXTURE_2D) != 0;
    let gl_rectangle = glIsEnabled(GL_TEXTURE_RECTANGLE) != 0;
    let point_smooth = glIsEnabled(GL_POINT_SMOOTH) != 0;
    glGetFloatv(GL_POINT_SIZE, &mut point_size);

    // Set drawing state
    glEnable(GL_BLEND);
    glBlendFunc(GL_SRC_ALPHA, GL_ONE_MINUS_SRC_ALPHA);
    glDisable(GL_TEXTURE_2D);
    glEnable(GL_POINT_SMOOTH);
    glHint(GL_POINT_SMOOTH_HINT, GL_NICEST);

    glPushMatrix();
    glLoadIdentity();

    // Draw the point
    glRasterPos2f(x, y);
    glPointSize(radius);
    glBegin(GL_POINTS);
    glVertex3f(x, y, z);
    glEnd();
    glFlush();

    // Restore OpenGL state
    glPopMatrix();

    if !gl_blend {
        glDisable(GL_BLEND);
    }
    if gl_texture_2d {
        glEnable(GL_TEXTURE_2D);
    }
    if gl_rectangle {
        glEnable(GL_TEXTURE_RECTANGLE);
    }
    if !point_smooth {
        glDisable(GL_POINT_SMOOTH);
    }

    glPointSize(point_size);
}
