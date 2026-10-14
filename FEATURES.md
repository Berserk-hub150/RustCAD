# Scope and remaining work

The working implementation provides a subset of 2D drafting, geometric editing, layers, document history, file conversion and a basic 3D wireframe view.

## Not implemented

- Full DWG reading/writing or full DXF fidelity.
- Associative dimensions, constraints and a parametric solver.
- SPLINE/NURBS and accurate general curve intersections.
- General trim/extend, fillets, chamfers, joined offsets and hatch fills.
- Reusable blocks, attributes, external references and block libraries.
- Paper space, layouts, plot styles, printer configuration and page setup.
- A 3D boundary-representation solid kernel.
- Extrusions, revolutions, lofts, sweeps and robust solid Boolean operations.
- Shaded/material rendering and photorealistic rendering.
- BIM, geographic information and industry-specific toolsets.
- AutoLISP, .NET/ObjectARX plug-ins and AutoCAD API compatibility.
- DWG recovery, crash recovery and enterprise-scale drawing benchmarks.
- Production-certified geometry tolerances and manufacturing validation.

## Suggested next milestones

1. Expand independent DXF fixtures, geometry tests and document validation.
2. Add properties, line types, spatial indexing and performance benchmarks.
3. Add robust curve intersections, trim/extend and fillet/chamfer.
4. Add interoperable dimensions, blocks and hatches.
5. Add paper space and accurate scaled PDF/printing.
6. Evaluate an existing open-source solid kernel for true 3D modeling.

AutoCAD-level functionality and interoperability require sustained engineering and validation.
