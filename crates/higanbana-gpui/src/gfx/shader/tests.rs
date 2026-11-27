//! Testes do compositor/tradutor WGSL (sem GPU: nada é desenhado aqui).
use super::*;

const RED: &str = "fn shade(p: ShadeInput) -> vec4<f32> { return vec4(1.0, 0.0, 0.0, 1.0); }";

#[test]
fn shader_params_macro_preenche_so_o_inicio() {
    let p: ShaderParams = crate::params![[1., 0., 0., 1.]];
    assert_eq!(p[0], [1., 0., 0., 1.]);
    assert!(p[1..].iter().all(|v| *v == [0.0; 4]));
}

#[test]
fn shader_params_macro_aceita_cor() {
    let p = crate::params![crate::rgb(0x0000ff), [0.5, 0., 0., 0.]];
    assert_eq!(p[0], [0., 0., 1., 1.]);
    assert_eq!(p[1], [0.5, 0., 0., 0.]);
}

#[test]
fn shader_flags_do_construtor() {
    // Sem a feature `shaders` não dá para criar; o teste só vale com ela.
    if let Ok(s) = Shader::wgsl(RED) {
        assert!(!s.is_backdrop() && !s.is_animated());
        let s = s.backdrop(true).animated(true);
        assert!(s.is_backdrop() && s.is_animated());
        assert_eq!(s.last_error(), None);
    }
}

#[cfg(not(feature = "shaders"))]
#[test]
fn shader_sem_feature_e_unsupported() {
    assert_eq!(Shader::wgsl(RED).err(), Some(ShaderError::Unsupported));
}

#[cfg(feature = "shaders")]
mod com_naga {
    use super::*;

    #[test]
    fn shader_traduz_para_todos_os_alvos() {
        let s = Shader::wgsl(RED).expect("shader válido");
        let t = s.translations();
        assert!(t.hlsl.contains("vs_main") && t.hlsl.contains("ps_main"), "{}", t.hlsl);
        assert!(t.glsl_vs.starts_with("#version 330"), "{}", t.glsl_vs);
        assert!(t.glsl_fs.starts_with("#version 330"), "{}", t.glsl_fs);
        assert_eq!(t.spirv.first(), Some(&0x0723_0203));
        assert!(t.wgsl.contains("fn vs_main") && t.wgsl.contains("fn ps_main") && t.wgsl.contains(RED));
    }

    #[test]
    fn shader_hlsl_usa_registradores_fixos() {
        let s = Shader::wgsl(RED).unwrap();
        let h = &s.translations().hlsl;
        assert!(h.contains("register(b0)"), "{h}");
        assert!(h.contains("register(t1)"), "{h}");
        assert!(h.contains("register(s2)"), "{h}");
        assert!(!h.contains("nagaSamplerHeap") && !h.contains("space"), "{h}");
    }

    #[test]
    fn shader_glsl_tem_nomes_documentados() {
        let s = Shader::wgsl(RED).unwrap();
        let t = s.translations();
        assert!(t.glsl_vs.contains(GLSL_UNIFORM_BLOCK_VS), "{}", t.glsl_vs);
        assert!(t.glsl_fs.contains(GLSL_UNIFORM_BLOCK_FS), "{}", t.glsl_fs);
        assert!(t.glsl_fs.contains(&format!("uniform sampler2D {GLSL_BACKDROP_SAMPLER};")), "{}", t.glsl_fs);
    }

    #[test]
    fn shader_erro_de_sintaxe_aponta_linha_do_usuario() {
        let src = "fn ajuda() -> f32 { return 1.0; }\n\nfn shade(p: ShadeInput) -> vec4<f32> { return vec4(1.0 ; }\n";
        match Shader::wgsl(src) {
            Err(ShaderError::Parse { line, column, .. }) => {
                assert_eq!(line, 3);
                assert!(column > 0);
            }
            other => panic!("esperava Parse na linha 3, veio {other:?}"),
        }
    }

    #[test]
    fn shader_sem_shade_cita_a_funcao() {
        match Shader::wgsl("fn outra() -> f32 { return 1.0; }") {
            Err(ShaderError::Parse { message, .. } | ShaderError::Validate { message, .. }) => {
                assert!(message.contains("shade"), "{message}")
            }
            other => panic!("esperava erro citando shade, veio {other:?}"),
        }
    }

    #[test]
    fn shader_recusa_binding_do_usuario() {
        let src = format!("@group(0) @binding(7) var<uniform> extra: vec4<f32>;\n{RED}");
        match Shader::wgsl(&src) {
            Err(ShaderError::Validate { line, message, .. }) => {
                assert_eq!(line, 1);
                assert!(message.contains("binding"), "{message}");
            }
            other => panic!("esperava Validate, veio {other:?}"),
        }
    }

    #[test]
    fn shader_compose_conta_linhas_do_preludio() {
        let (full, lines) = compose::compose("fn shade(p: ShadeInput) -> vec4<f32> { return vec4(0.0); }");
        let user_line = full.lines().nth(lines as usize).unwrap();
        assert!(user_line.starts_with("fn shade"), "{user_line}");
    }

    #[test]
    fn shader_usa_backdrop_e_params() {
        let src = "fn shade(p: ShadeInput) -> vec4<f32> {\n  let b = backdrop_blur(p.screen, param(0u).x) + backdrop(p.screen);\n  return b * rounded_sdf(p.px, p.size, p.radius) + vec4(p.uv, p.time, p.scale);\n}";
        let s = Shader::wgsl(src).expect("prelúdio completo");
        assert!(!s.translations().hlsl.is_empty());
    }

    #[test]
    fn shader_vai_para_o_core_e_para_a_draw_list() {
        // Sem GPU utilizável o teste não tem o que checar.
        let Ok(ctx) = crate::Context::new(crate::BackendKind::Auto) else { return };
        let s = Shader::wgsl(RED).unwrap().backdrop(true);
        let gpu = ctx.shader(&s).expect("hg_shader_create");
        assert_eq!(gpu.native_error(), None);
        let mut dl = crate::DrawList::new();
        let d = ShaderDraw { rect: crate::Rect::new(10., 10., 100., 50.), params: crate::params![[1., 2., 3., 4.]], ..Default::default() };
        dl.shader(&gpu, &d).expect("hg_draw_shader");
    }
}
