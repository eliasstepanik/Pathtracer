# Pathtracer in Rust

A physically-based path tracer written in Rust, designed to simulate realistic lighting and materials by tracing the paths of light as pixels in an image and simulating the effects when they encounter virtual objects. This project supports both CPU and GPU rendering and includes integration with Blender for scene creation.

## Features

- **Physically-Based Rendering (PBR):** Implements realistic light scattering, reflection, refraction, and volumetric effects.
- **GGX Microfacet Model:** Uses the GGX model for specular reflection.
- **Multiple Materials and Objects:** Supports spheres and planes with customizable materials.
- **Area Lights:** Implements area lights for soft shadows and realistic lighting.
- **Depth of Field:** Includes support for camera aperture and focus distance.
- **GPU Acceleration:** Offers GPU rendering using WGPU for faster performance.
- **Blender Integration:** Comes with a Blender add-on (`ray_scene_builder.py`) to build and export scenes.
- **Configurable Rendering Parameters:** Adjustable samples per pixel, resolution, and maximum ray bounces.

## Table of Contents

- [Requirements](#requirements)
- [Installation](#installation)
- [Usage](#usage)
    - [Running the Renderer](#running-the-renderer)
    - [Command-Line Options](#command-line-options)
- [Scene Description](#scene-description)
    - [Materials](#materials)
    - [Objects](#objects)
    - [Lights](#lights)
    - [Camera](#camera)
    - [Render Settings](#render-settings)
- [Blender Integration](#blender-integration)
    - [Installation of Blender Add-on](#installation-of-blender-add-on)
    - [Using the Add-on](#using-the-add-on)
- [GPU Rendering](#gpu-rendering)
- [Examples](#examples)
- [Contributing](#contributing)
- [License](#license)

## Requirements

- **Rust Compiler:** Rust 1.60 or later. Install from [rust-lang.org](https://www.rust-lang.org/tools/install).
- **Cargo:** Comes with Rust installation.
- **Blender (Optional):** Blender 3.2 or later for scene creation using the add-on.
- **wgpu Dependencies:** Ensure that your system supports Vulkan, Metal, DX12, or other backends supported by `wgpu`.

## Installation

1. **Clone the Repository:**

   ```bash
   git clone https://github.com/yourusername/pathtracer.git
   cd pathtracer
   ```

2. **Build the Project:**

   ```bash
   cargo build --release
   ```

   This will compile the project in release mode for optimal performance.

## Usage

### Running the Renderer

By default, the renderer will look for a `scene.json` file in the current directory and render the scene described.

```bash
cargo run --release
```

This command will compile (if not already compiled) and run the renderer.

### Command-Line Options

- `--quiet` or `-q`: Run the renderer without progress output.
- `--gpu`: Enable GPU rendering mode for faster performance.

Example:

```bash
cargo run --release -- --gpu
```

*Note: The `--` is used to separate cargo's options from the program's options.*

## Scene Description

The renderer uses a `scene.json` file to describe the scene. This file includes definitions for materials, objects, lights, camera settings, and render settings.

### Materials

Materials define the surface properties of objects.

```json
"materials": {
  "gold": {
    "rgb": [1.0, 0.766, 0.336],
    "metallic": 1.0,
    "roughness": 0.2,
    "ior": 0.0,
    "volume_density": 0.0,
    "volume_anisotropy": 0.0
  },
  "glass": {
    "rgb": [1.0, 1.0, 1.0],
    "metallic": 0.0,
    "roughness": 0.01,
    "ior": 1.5,
    "volume_density": 0.0,
    "volume_anisotropy": 0.0
  }
}
```

- `rgb`: Base color of the material.
- `metallic`: Degree to which the material is metallic (0.0 to 1.0).
- `roughness`: Surface roughness (0.01 to 1.0).
- `ior`: Index of refraction for transparent materials (>1.0).
- `volume_density`: Density for volumetric scattering (e.g., fog).
- `volume_anisotropy`: Scattering direction (-1.0 to 1.0).

### Objects

Objects can be spheres or planes with assigned materials.

```json
"objects": [
  {
    "sphere": {
      "name": "GlassSphere",
      "center": [0.0, 1.0, 0.0],
      "radius": 1.0,
      "mat": "glass",
      "in_focus": true
    }
  },
  {
    "plane": {
      "name": "Ground",
      "point": [0.0, 0.0, 0.0],
      "u": [5.0, 0.0, 0.0],
      "v": [0.0, 0.0, 5.0],
      "mat": "gold",
      "in_focus": false
    }
  }
]
```

- `sphere`: Defines a sphere object.
    - `center`: XYZ coordinates of the sphere's center.
    - `radius`: Radius of the sphere.
- `plane`: Defines a plane object.
    - `point`: A point on the plane.
    - `u`, `v`: Edge vectors defining the plane's size and orientation.
- `name`: An identifier for the object.
- `mat`: The material name assigned to the object.
- `in_focus`: Boolean indicating if the object should be considered in autofocus calculations.

### Lights

Defines area lights in the scene.

```json
"lights": [
  {
    "pos": [0.0, 5.0, 0.0],
    "u": [2.0, 0.0, 0.0],
    "v": [0.0, 0.0, 2.0],
    "intensity": [25.0, 25.0, 25.0]
  }
]
```

- `pos`: Center position of the light.
- `u`, `v`: Edge vectors defining the light's size and orientation.
- `intensity`: RGB intensity of the light.

### Camera

Defines the camera settings.

```json
"camera": {
  "pos": [0.0, 2.0, -5.0],
  "look_at": [0.0, 1.0, 0.0],
  "up": [0.0, 1.0, 0.0],
  "fov": 45.0,
  "aperture": 0.01
}
```

- `pos`: Camera position.
- `look_at`: Point the camera is looking at.
- `up`: Up direction for the camera.
- `fov`: Field of view in degrees.
- `aperture`: Size of the camera's aperture (controls depth of field).

### Render Settings

Controls the rendering parameters.

```json
"render": {
  "width": 800,
  "height": 600,
  "samples": 128,
  "gpu_workload": 40000000
}
```

- `width`, `height`: Dimensions of the output image.
- `samples`: Number of samples per pixel.
- `gpu_workload`: Maximum GPU workload per dispatch. Lower values reduce GPU usage if your driver is unstable.

## Blender Integration

A Blender add-on (`ray_scene_builder.py`) is included to facilitate scene creation.

### Installation of Blender Add-on

1. **Copy the Add-on File:**

   Place `ray_scene_builder.py` in a known location on your system.

2. **Install the Add-on:**

    - Open Blender.
    - Go to **Edit > Preferences > Add-ons**.
    - Click **Install...** and select `ray_scene_builder.py`.
    - Enable the add-on by checking the box next to **Rust Pathtracer Scene Builder**.

### Using the Add-on

- Access the add-on panel under **3D View > Sidebar > Ray Scene**.
- Use the panel to:
    - Add spheres, planes, and area lights compatible with the path tracer.
    - Set material properties such as color, metallic, roughness, and IOR.
    - Mark objects as "In Focus" for autofocus calculations.
- **Export Scene:**
    - After setting up the scene, click **Export Scene** in the panel.
    - Choose a location to save `scene.json`.
- **Import Scene:**
    - Use **Import Scene** to load an existing `scene.json` into Blender for adjustments.

## GPU Rendering

To leverage GPU acceleration, run the renderer with the `--gpu` flag:

```bash
cargo run --release -- --gpu
```

**Requirements for GPU Rendering:**

- A GPU with support for Vulkan, Metal, or DirectX 12.
- The **wgpu** library dependencies must be met on your system.

**Notes:**

- GPU rendering can significantly speed up rendering times.
- Ensure that your system's GPU drivers are up to date.


### Example

![Glossy Metallic Spheres](render_7680x4320_s131072_ap0.02_f10.0_tzW1Af.png)

- **Description:** Two metallic spheres with varying roughness. As well as a glass sphere in the front.
- **Settings:** 7680x4320 resolution, 131072 samples per pixel.


## Contributing

Contributions are welcome! Feel free to open issues or submit pull requests.

To contribute:

1. Fork the repository.
2. Create a new branch for your feature or bugfix.
3. Commit your changes with descriptive messages.
4. Open a pull request describing your changes.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

---

*Happy Rendering! If you have any questions or need assistance, please feel free to reach out.*