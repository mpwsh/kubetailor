use anyhow::Result;
use handlebars::{
    Context, DirectorySourceOptions, Handlebars, Helper, HelperResult, Output, RenderContext,
};

pub fn create_handlebars() -> Result<Handlebars<'static>> {
    let mut handlebars = Handlebars::new();

    // Register templates directory
    handlebars
        .register_templates_directory("./web/templates", DirectorySourceOptions::default())?;

    // Register helpers
    register_helpers(&mut handlebars);

    Ok(handlebars)
}

fn register_helpers(handlebars: &mut Handlebars) {
    // Concat helper
    handlebars.register_helper(
        "concat",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &Context,
             _: &mut RenderContext,
             out: &mut dyn Output|
             -> HelperResult {
                let param0 = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
                let param1 = h.param(1).and_then(|v| v.value().as_str()).unwrap_or("");
                out.write(&format!("{}{}", param0, param1))?;
                Ok(())
            },
        ),
    );

    // Add helper
    handlebars.register_helper(
        "add",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &Context,
             _: &mut RenderContext,
             out: &mut dyn Output|
             -> HelperResult {
                let param0 = h.param(0).and_then(|v| v.value().as_i64()).unwrap_or(0);
                let param1 = h.param(1).and_then(|v| v.value().as_i64()).unwrap_or(0);
                out.write(&(param0 + param1).to_string())?;
                Ok(())
            },
        ),
    );

    // Eq helper
    handlebars.register_helper(
        "eq",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &Context,
             _: &mut RenderContext,
             out: &mut dyn Output|
             -> HelperResult {
                let param0 = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
                let param1 = h.param(1).and_then(|v| v.value().as_str()).unwrap_or("");
                out.write(if param0 == param1 { "true" } else { "false" })?;
                Ok(())
            },
        ),
    );

    // Default helper
    handlebars.register_helper(
        "default",
        Box::new(
            |h: &Helper,
             _: &Handlebars,
             _: &Context,
             _: &mut RenderContext,
             out: &mut dyn Output|
             -> HelperResult {
                let value = h.param(0).and_then(|v| v.value().as_str()).unwrap_or("");
                let default = h.param(1).and_then(|v| v.value().as_str()).unwrap_or("");
                out.write(if value.is_empty() { default } else { value })?;
                Ok(())
            },
        ),
    );
}
