use rust_cad_core::*;
use std::{env, fs, path::Path, process};
fn run() -> Result<(), String> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(|x| x.as_str()) == Some("sample") {
        let dir = Path::new(args.get(1).map(|s| s.as_str()).unwrap_or("."));
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let d = Drawing::sample();
        for (name, text) in [
            ("plate.rcad", save_project(&d)),
            ("plate.dxf", export_dxf(&d)),
            ("plate.svg", export_svg(&d)),
        ] {
            fs::write(dir.join(name), text).map_err(|e| e.to_string())?
        }
        println!("Created plate.rcad, plate.dxf and plate.svg");
        return Ok(());
    }
    if args.len() != 3 || args[0] != "convert" {
        return Err("Usage: rust-cad-cli sample [directory]\n       rust-cad-cli convert input.rcad|input.dxf output.rcad|output.dxf|output.svg|output.stl".into());
    }
    let text = fs::read_to_string(&args[1]).map_err(|e| e.to_string())?;
    let drawing = if args[1].to_lowercase().ends_with(".dxf") {
        let r = import_dxf(&text)?;
        for w in r.warnings {
            eprintln!("Warning: {w}")
        }
        r.drawing
    } else {
        load_project(&text)?
    };
    let out = match Path::new(&args[2]).extension().and_then(|e| e.to_str()) {
        Some("rcad") => save_project(&drawing),
        Some("dxf") => export_dxf(&drawing),
        Some("svg") => export_svg(&drawing),
        Some("stl") => export_stl(&drawing)?,
        _ => return Err("Unsupported output extension".into()),
    };
    fs::write(&args[2], out).map_err(|e| e.to_string())?;
    println!(
        "Converted {} entities to {}",
        drawing.entities.len(),
        args[2]
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        process::exit(1)
    }
}
