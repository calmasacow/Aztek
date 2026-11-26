<p align="center"><img src="docs/splash.png" alt="Aztek"></p>

<p align="center">
  <a href="https://github.com/srlegrand/Aztek/releases"><img src="https://img.shields.io/badge/Download-Windows%20%7C%20Linux%20%7C%20macOS-white?style=for-the-badge" alt="Download"></a>
  <a href="https://github.com/srlegrand/Aztek/fork"><img src="https://img.shields.io/badge/Fork%20it%20on%20GitHub-white?style=for-the-badge&logo=github&logoColor=black" alt="Fork it on GitHub"></a>
  <a href="#copy-it-anywhere"><img src="https://img.shields.io/badge/Copy%20it%20anywhere-white?style=for-the-badge" alt="Copy it anywhere"></a>
  <a href="#fix-it-yourself"><img src="https://img.shields.io/badge/Fix%20it%20yourself-white?style=for-the-badge" alt="Fix it yourself"></a>
  <a href="https://srlegrand.github.io/Aztek/"><img src="https://img.shields.io/badge/Website-white?style=for-the-badge" alt="Website"></a>
  <a href="https://discord.gg/XDEgpWDVKa"><img src="https://img.shields.io/badge/Join%20the%20Discord-5865F2?style=for-the-badge&logo=discord&logoColor=white" alt="Join the Discord"></a>
</p>

## Build Linux

    git clone https://github.com/srlegrand/Aztek.git
    cd Aztek
    ./build.sh --deps --run

## Build Windows

    git clone https://github.com/srlegrand/Aztek.git
    cd Aztek
    .\build.bat -Run

From PowerShell, with Visual Studio 2022.

## Build Mac

    git clone https://github.com/srlegrand/Aztek.git
    cd Aztek
    ./build-macos.sh --run

On a new Mac, `git clone` asks to install Xcode's command line tools first: install them, then run it again. The
script installs whatever else is missing (Homebrew, which asks for your password, CMake, Ninja, MoltenVK and Rust).

What you build yourself runs straight away: Windows and macOS only check programs that were downloaded. Prebuilt
packages are under [Install](#install), and [Running unsigned builds](#running-unsigned-builds) says how to open them.

<details>
<summary>Build details for Linux, macOS and Windows</summary>

**Linux.** `--deps` installs the build tools (apt, dnf or pacman) and only needs running once; `--run` starts Aztek
afterwards; `--install` puts the app in `~/.local/share/aztek` with a launcher in `~/.local/bin`, a menu entry and an
icon; `--debug` builds into `build-debug/`. Rust comes from [rustup](https://rustup.rs). An active conda environment is
fine: the script ignores conda's libraries.

Every build is self-contained and follows the VFX Reference Platform CY2027: Python 3.13, Qt and PySide6 6.8, C++20.
The first one fetches Qt 6.8.3 from Qt's own kit with
[aqtinstall](https://github.com/miurahr/aqtinstall) into `build/qt`, and a relocatable Python 3.13 from
[python-build-standalone](https://github.com/astral-sh/python-build-standalone) into `build/python` with
`tools/python/fetch-python.sh`, checked against its SHA-256. It then installs PySide6 and shiboken6 6.8.3, the release
built for that Qt, into that Python, builds against all three and stages the result in `build/Aztek`. 6.8.3's Linux
wheels need glibc 2.28, so the same versions build on Rocky 8. Nothing comes from the system's Qt or Python; later builds reuse the fetched ones, and
`tools/bundle/versions.sh` holds the versions.

Run what was built with `build/Aztek/AppRun`, not `build/aztek`: AppRun points the program at its own Qt and Python.
A build folder that was once configured against the system's Qt is configured again against the fetched one; to
start completely afresh, `rm -rf build build-debug` and run the script again. For a crash, `./build.sh --debug` keeps
its symbols in `build-debug/Aztek`, and `gdb -ex run -ex bt --args sh build-debug/Aztek/AppRun` prints a backtrace
with Aztek's own functions named.

**macOS.** The script installs what is missing before it builds: Xcode's command line tools, [Homebrew](https://brew.sh)
(added to `~/.zprofile` when it isn't on the path), CMake, Ninja, MoltenVK and Rust; `--deps` also updates them. Every build fetches Qt 6.8.3, Python 3.13 and PySide6 6.8.3 and puts them, with MoltenVK, inside
`build/aztek.app`, signed ad hoc, or with your Developer ID when `AZTEK_SIGN_IDENTITY` names one (docs/PACKAGING.md). `--package` also makes `dist/aztek-macos-<arch>.zip`, and `--install` puts Aztek.app
in `~/Applications`. macOS is new: the viewport, menus and shortcuts haven't been checked there yet.

**Windows**, from PowerShell or a command prompt: Visual Studio 2022 (or its Build Tools) with "Desktop development with
C++", Rust from [rustup](https://rustup.rs) and CMake. Every build fetches Qt 6.8.3 for MSVC 2022, Python 3.13 and PySide6 6.8.3 into `build\`, builds against them, and makes a folder in
`build\dist` that runs by itself: Qt's DLLs, the VC runtime, Python and PySide6 beside `aztek.exe`.

**By hand**, on any of them, after the build script has fetched them: CMake needs the kit `tools/bundle/versions.sh`
names (`build/qt/6.8.3/gcc_64`) on its `CMAKE_PREFIX_PATH`, and
`build/python/bin/python3.13` as `Python3_EXECUTABLE` (on macOS the kit's folder is `macos`). The first build compiles
the Rust core (a few minutes); later builds only rebuild what changed. `-DAZTEK_PYTHON=OFF` builds without Python.

Options:

    aztek [scene.ma|scene.mb] [--script file.mel|file.py] [--screenshot out.png]

On KDE Plasma, Alt + drag moves windows by default: set System Settings > Window Management > Window Behavior >
Window Actions > Modifier key to Meta.

</details>

## This doesn't feel right!

Aztek should feel like second nature to anyone who has spent years in a 3D package. If something doesn't work, or works but
doesn't feel quite right (a hotkey, a drag, a default, a menu in the wrong place), press the orange **This doesn't feel
right!** button at the far right of the shelf bar. Say what you did and what you expected; that's the whole report. It opens a filled-in
GitHub issue, so every rough edge anyone notices ends up in one public list.

![The This doesn't feel right! window over the main window](docs/feedback.png)

It is also under Help > This doesn't feel right!, and the `AztekFeedback` runtime command. The small arrow on the
button's left folds it away, and brings it back; Aztek remembers which. The window's Attachments section has a box for each kind,
all optional:

- **Pictures**: the screenshot taken as the window opened (click it to add it), more screenshots after a delay, and images of your own,
  added from a file, pasted (Ctrl+V in the description works too) or dropped on the window. Click a picture to draw on it with a pen,
  arrows, rectangles and text, crop it or retake it;
- **Video**: Record takes 3, 5 or 10 seconds of Aztek's windows at 720p after a delay you set, with your pointer, clicks and keys drawn
  on, kept under GitHub's 10 MB; or attach a video of your own (`.mp4`, `.mov` or `.webm`, up to 10 MB) from a file or by dropping it;
- **Scene**: a zipped `.ma` copy of the current scene. Your scene stays as it is, and the copy has a generic name. It holds the names of
  everything in the scene and may hold file paths. Referenced files and textures are not included.

Nothing is attached until you add it, and the first of each kind asks you to confirm. **Reports are public: never include work under NDA or anything confidential. If you do, it
is at your own risk.**

Sending a report makes no network request from the app. "Open on GitHub" opens a filled-in issue page in your browser, puts the picture on the clipboard (Ctrl+V
in the description box) and shows the folder holding the attachments, under `feedback/` in the app's data location, so the zip can be dragged
in. GitHub plays the video in the issue. You need a GitHub account, and nothing is public until you press Submit on that page. The window shows exactly what the page will be
filled in with, folded under the attachments. GitHub takes files up to 25 MB and images up to 10 MB.

## Fix it yourself

If something is wrong and you can see how to fix it, fix it and send the change. Small fixes are as welcome as large
ones, and there is nothing to ask first. Three ways. **Click one to unfold it:**

[CONTRIBUTING.md](CONTRIBUTING.md) has the short version, with how to say you are on an issue, what a change carries
and what happens after you send it.

<details>
<summary><h3>By hand: build it, change it, send a patch or a pull request</h3></summary>

1. **Get the tools, the source, and build it.** You need `git`, and `gh` (GitHub's command line tool) if you will
   send a pull request. What the build itself needs is under [Build](#build).

   Linux:

       sudo apt install git
       git clone https://github.com/srlegrand/Aztek.git
       cd Aztek
       ./build.sh --deps --run

   On Fedora or Rocky it is `sudo dnf install git`, on Arch `sudo pacman -S git`. For `gh`, follow [its Linux instructions](https://github.com/cli/cli/blob/trunk/docs/install_linux.md): the `gh`
   that Debian and Ubuntu ship themselves is too old to work with GitHub (as of 11/2025, by its maintainers' note).

   macOS:

       git clone https://github.com/srlegrand/Aztek.git
       cd Aztek
       ./build-macos.sh --run

   `git clone` asks to install Xcode's command line tools the first time: install them and run it again. The build
   script installs Homebrew and the rest. For `gh`, run `brew install gh` in a new terminal.

   Windows, from PowerShell, with Visual Studio 2022 and Qt installed as [Build](#build) says:

       winget install --id Git.Git
       winget install --id GitHub.cli
       git clone https://github.com/srlegrand/Aztek.git
       cd Aztek
       .\build.bat -Run

   Open a new terminal after installing so `git` and `gh` are found. Then, once, sign `gh` in to your GitHub account
   (a free one will do; make it at [github.com/signup](https://github.com/signup)):

       gh auth login

   Answer GitHub.com, HTTPS, Yes to authenticating Git, and "Login with a web browser": it shows a code, opens the
   browser, and you paste the code there. `gh auth status` says whether it worked. A patch attached to an issue needs
   no `gh`, only the account.

2. **Find the code.** The Rust core is under `crates/`: `aztek_scene` holds the nodes and commands, `aztek_geo` the mesh
   operations, `aztek_host` the viewport and everything the interface asks of the core. The Qt interface is under
   `app/src`. Searching for a command's name, a menu label or an error message usually lands on the right file.
   [PARITY.md](PARITY.md) says how each area is meant to behave.

3. **Make the change and check it.** `./build.sh --run` rebuilds and starts Aztek (`./build-macos.sh --run` on macOS,
   `.\build.bat -Run` on Windows). Each crate keeps its tests in its
   `tests/` folder; run the ones for the crate you touched, and add one beside them if the fix can be tested.

       cargo test -p aztek_scene

4. **Send it**, as a patch or as a pull request.

   A patch needs no fork. Make one file holding everything you changed, new files included:

       git add -A
       git diff --cached > my-fix.patch

   Send it with the **Send a patch** button under [Send your fix](#send-your-fix).

   A pull request, with `gh` signed in. The first command makes your own copy of the repository on GitHub and points
   this folder at it; the last pushes your branch there and opens the pull request:

       gh repo fork --remote
       git checkout -b my-fix
       git add -A
       git commit -m "What the fix does"
       gh pr create --fill

   If git asks who you are at the commit, tell it once: `git config --global user.name "Your Name"` and
   `git config --global user.email "you@example.com"`.

</details>

<details>
<summary><h3>With an LLM, nothing to install: the source zip and a chat</h3></summary>


No git, no build: a chat service that takes a file and can give one back is enough.

1. Download the source: [Aztek-main.zip](https://github.com/srlegrand/Aztek/archive/refs/heads/main.zip) (about 30 MB).
2. Start a new chat, attach the zip, and send this with the problem filled in. If the service refuses the zip for its
   size, unzip it and zip only `crates`, `app`, `CLAUDE.md` and `PARITY.md` (about 5 MB).

   ```text
   The attached zip is the source of Aztek (https://github.com/srlegrand/Aztek), a 3D application
   with a Rust core under crates/ and a Qt interface in C++ under app/src. Unzipped, everything
   sits in one top folder (Aztek-main).

   Read CLAUDE.md first: it holds the project's rules and layout. PARITY.md says how each area
   is meant to behave.

   The problem:
   <What you did, what happened, and what you expected instead. Paste the issue's text and
   its number if there is one.>

   Do this, in order:
   1. Find the code responsible and tell me what causes the problem.
   2. Make the smallest change that fixes it, in the style of the code around it. Don't
      refactor, rename or reformat what the fix doesn't need.
   3. If the fix can be tested, add a test in that crate's tests/ folder.
   4. If the change alters behaviour that PARITY.md describes, update PARITY.md.

   Give me the result as one file to download, named my-fix.patch: a unified diff that
   `git apply` accepts, with paths from the repository's root (a/crates/..., b/crates/...,
   without the Aztek-main folder), holding only the lines that change and their context. Add
   no credit or attribution lines anywhere. If you can't make a file, put the whole diff in
   one code block.

   Then tell me, in a few lines: the files you changed and why, how to check the fix by hand
   in the app, and what you could not build, run or verify.
   ```

3. Save what it gives back as `my-fix.patch`, and send it with the **Send a patch** button under
   [Send your fix](#send-your-fix). You need a GitHub account for that, nothing else.
4. On that page, choose "With an LLM from the source zip, not built or run": nobody has yet seen it work, so it gets
   checked before it goes in.

</details>

<details>
<summary><h3>With an LLM, in the built source</h3></summary>


A coding assistant that can read the folder and run commands in it can find the code and make the change with you, and build
and test what it changes. Clone and build first (step 1 of By hand), start the assistant in the `Aztek` folder, and give it this, with the
problem filled in:

```text
You are working in a clone of Aztek (https://github.com/srlegrand/Aztek), a 3D application
with a Rust core under crates/ and a Qt interface in C++ under app/src.

Read CLAUDE.md first: it holds the project's rules and layout. PARITY.md says how each area
is meant to behave.

The problem:
<What you did, what happened, and what you expected instead. Paste the issue's text and
its number if there is one.>

Do this, in order:
1. Find the code responsible and tell me what causes the problem before changing anything.
2. Make the smallest change that fixes it, in the style of the code around it. Don't
   refactor, rename or reformat what the fix doesn't need.
3. If the fix can be tested, add a test in that crate's tests/ folder, and run the crate's
   tests: cargo test -p <crate>.
4. Build with ./build.sh (build.bat on Windows, ./build-macos.sh on macOS) and fix what
   doesn't compile.
5. If the change alters behaviour that PARITY.md describes, update PARITY.md.
6. Don't commit or push, and add no credit or attribution lines anywhere.

When you are done:
- run: git add -A && git diff --cached > my-fix.patch
- list the files you changed and why, in a few lines
- give me the steps to check the fix by hand in the app
- say plainly what you could not test or verify
```

Then build, start Aztek and check the fix yourself by the steps it gives: it can be wrong while sounding sure. Read the
patch before sending it (step 4 of By hand), and say in the issue what you checked.

</details>

### Send your fix

<p>
  <a href="https://github.com/srlegrand/Aztek/issues/new?template=patch.yml&title=Patch%3A+"><img src="https://img.shields.io/badge/Send%20a%20patch-f08a1c?style=for-the-badge" alt="Send a patch"></a>
  <a href="https://srlegrand.github.io/Aztek/#send"><img src="https://img.shields.io/badge/Open%20a%20pull%20request-1d5f9e?style=for-the-badge" alt="Open a pull request"></a>
</p>

- **Send a patch**: you have `my-fix.patch` and a GitHub account, nothing else. The button opens a form on GitHub with
  its fields laid out. Say what it fixes, drag the patch file into the box named The patch, and press Submit. A pull
  request is opened from the patch and linked on that page a minute or two later. If the patch doesn't apply, the page
  says why; edit it with a corrected patch and it is tried again.
- **Open a pull request**: you pushed a branch to your fork. The button goes to a short form on the website: give your
  GitHub user name and branch, and it opens the pull request page on GitHub filled in. Press Create pull request there.
- From a patch file to a pull request in a terminal, with `gh` signed in (step 1 of By hand):

      gh repo fork srlegrand/Aztek --clone
      cd Aztek
      git checkout -b my-fix
      git apply ~/Downloads/my-fix.patch
      git add -A
      git commit -m "What the fix does"
      gh pr create --fill

What helps it go in quickly: one fix per patch, a line on what was wrong and what happens now, and the issue's number
if there is one. A change you send goes in under [MIT-0](LICENSE) like everything else here. And if you would rather
keep the fix in your own copy, that is fine too: see [Copy it anywhere](#copy-it-anywhere).

## Why

The tools we build our work on shouldn't belong to whoever bought them last. Too many of them were bought, left to stand still, and kept on subscription while bugs older than some of the people using them went unfixed. Aztek works the way you already work, and it belongs to everyone who uses it: no account, no subscription, no owner who can sell it, shelve it or switch it off.

**Back it up, clone it, fork it.** As long as one copy exists somewhere, nobody can take it away. See [Copy it anywhere](#copy-it-anywhere).

[![Build](https://github.com/srlegrand/Aztek/actions/workflows/build.yml/badge.svg)](https://github.com/srlegrand/Aztek/actions/workflows/build.yml)

A 3D application for modelling, UVs, rigging, animation, dynamics and rendering, with the industry-standard commands, hotkeys, menus and file format. Rust core (scene graph, commands,
geometry, Vulkan viewport), Qt Widgets shell in C++, a MEL interpreter, and embedded Python with `aztek.cmds`,
compatibility modules so existing scripts import unchanged, and the Python API, OpenAztek (`aztek.api.OpenAztek`, and 1.0 as
`aztek.OpenAztek`), which those scripts reach as `maya.api.OpenMaya` and `maya.OpenMaya`. Reads and writes `.ma` and `.mb` files; USD and OBJ come as plug-ins (mayaUsdPlugin, objExport), as in the reference. [PARITY.md](PARITY.md) has the behaviour
target and status, including known differences; [docs/COMMANDS.md](docs/COMMANDS.md) lists all 1328 commands;
[docs/ENVIRONMENT.md](docs/ENVIRONMENT.md) lists the environment variables and which are read.

Website: [srlegrand.github.io/Aztek](https://srlegrand.github.io/Aztek/). Licence: [MIT-0](LICENSE).

**Download** from the [releases page](https://github.com/srlegrand/Aztek/releases): Windows installer or zip, Rocky Linux 9 and 8 `.rpm`, Ubuntu 26.04 and 24.04 `.deb`, an AppImage for any other Linux (x86_64 and 64-bit ARM), Linux `.tar.gz`, and an early macOS build for Apple silicon. Install steps are under [Install](#install).

| Modelling history | A flag in a breeze | A tower knocked down |
| --- | --- | --- |
| ![Extrude, width and bevel keyed on their history nodes](docs/clip-modelling.gif) | ![A cloth flag nailed to its pole, in a breeze](docs/clip-flag.gif) | ![A tower of blocks knocked down by a heavy ball](docs/clip-tower.gif) |

![A production rig opened from its .ma file](docs/production-rig.png)

A production character rig, opened from its `.ma` file and posed: 414 joints, 37 skinClusters, 17
blendShapes, 82 IK handles, follicles, ribbons and wires. It opens in about 3 seconds and responds in 13 to 44 ms per change.

## Copy it anywhere

Copying Aztek somewhere else is welcome. Clone it, fork it, mirror it or re-host it on any git server or other version
control: Codeberg, GitLab, a forge in your country, a server at your studio, a drive in your pocket. With or without
changes, under your own name or none. There is nothing to ask and no one to credit: the licence is [MIT-0](LICENSE).
Every extra copy makes the project harder to lose.

    git clone --mirror https://github.com/srlegrand/Aztek.git
    cd Aztek.git
    git push --mirror <your repository's URL>

## What's in it

**Modelling**

- **Polygons:** the standard polygon commands with construction history; Quad Draw, Multi-Cut, Insert and Offset Edge
  Loop, Target Weld, Slide Edge, Crease, Create and Append Polygon, booleans; symmetry, soft selection; eighteen sculpting brushes.
- **Retopologize:** clean all-quad meshes, closed wherever the source is, with even loops and few irregular points;
  the reference's polyRetopo flags (face count, hard edges and edges by angle, feature tags, symmetry, uniformity,
  anisotropy, regularity). See [docs/RETOPOLOGY.md](docs/RETOPOLOGY.md).
- **NURBS:** curves (CV and EP tools, rebuild, attach, detach, offset) and surfaces (primitives, loft, revolve, extrude,
  boundary, birail, `nurbsToPoly`), with CVs as components.
- **Manipulators** on history and deformer nodes (T), with in-view numbers: extrude, projections, primitives, nonlinears, soft
  modification and more. Move along normals, custom axes, step snap, tweak mode.

**UVs**

- Projections, unfold, layout, cut and sew in the UV Editor, by UV, shell, edge or face.

**Rigging**

- **Skeletons and skinning:** joints (orient, mirror, insert, segment scale compensate), RP, SC and spline IK, every constraint, smooth
  skin with weight painting, mirror and copy, dual quaternion; HumanIK characters with definitions, T-pose checks, control rig and
  retargeting between skeletons of different sizes. Controls keep their colours: drawing overrides, templates, reference display,
  display layer colours and wireframe colours.

  ![A production rig opened from its .ma file with its textures, posed with its own controls](docs/clip-rig-pose.gif)

  ![A quadruped skeleton posed with the rotate tool](docs/quadruped-rig.png)
- **Deformers:** blendShape in full (Shape Editor, in-between and live targets), lattice, wrap, wire, delta mush, softMod,
  nonlinears, clusters, sculpt, jiggle, shrinkWrap, proximityWrap, curveWarp, wrinkle, morph and more; deformer weights painted
  with the Paint Attribute tool or exported.

**Animation**

- Keys with weighted tangents and infinities, Graph Editor, Dope Sheet, set driven keys, breakdowns,
  copy and paste keys, baking, motion paths, animation layers with their editor (additive, override, passthrough, merge, extract),
  character sets and clips, Time Slider bookmarks. **Playblast** to images or, through ffmpeg, to a movie.

  ![A keyed IK arm played back](docs/clip-arm.gif)

**Dynamics**

- One solver for cloth, particles and rigid bodies, which all collide with and push each other. Cloth
  on skinned or animated meshes with material presets, colliders, emitters, fields and constraints; it never explodes,
  and a vertex it cannot place leaves the simulation for a while and comes back, marked in the view. Older dynamics
  nodes in `.ma` files load into it. See [docs/DYNAMICS.md](docs/DYNAMICS.md).

  | Cloth, rendered with Harold | Rigid bodies on cloth | Particles |
  | --- | --- | --- |
  | ![A sheet dropped on a ball](docs/clip-cloth.gif) | ![Boxes dropped into a cloth hammock](docs/clip-rigid.gif) | ![Particles from an emitter with turbulence and a floor collider](docs/clip-particles.gif) |

**Rendering and look development**

- **Harold**, a physically based path tracer on the CPU: the standard surface with subsurface, coat, sheen,
  transmission and thin film; area, sky dome with HDR images, mesh and photometric lights; subdivision, displacement,
  motion blur and depth of field; light and data AOVs into multi-layer EXR; interactive rendering in the Render View as
  the scene changes, Render Settings, and batch rendering with no display (`aztek -render`). Every material and light
  in a scene renders through it. See [docs/HAROLD.md](docs/HAROLD.md).

  ![The Render View keeping up with the camera as it moves round a rig](docs/clip-ipr.gif)

  ![Two rigs and a row of materials rendered with Harold](docs/harold.png)
- **UV sets and glTF:** several UV sets to a mesh, each with its own seams, kept in `.ma` files; the UV Set Editor
  and a UV set list in the UV Editor. File > Import reads FBX (binary) as the reference's FBX plug-in does: hierarchy, joints with their orients, meshes with UVs, colour sets and hard edges, lambert and phong materials with file textures and normal maps, skins with their weights and bind pose, under the `FBXImport*` settings and the FBX Import Options; and glTF 2.0: hierarchy, meshes, UVs, materials, joints. File > Export writes FBX (binary 7.4: hierarchy, joints with their orients, polygons, UV sets, materials with colour, transparency and normal maps, skins, under the `FBXExport*` settings) and glTF 2.0 (`.glb` or `.gltf`): hierarchy, meshes, normals,
  UV sets, vertex colours, materials, colour textures, skins (joints, weights, bind matrices), blend shapes as morph
  targets and sampled animation. The file is Y up; a Z-up scene is converted with no node added, or kept Z up
  (`exportGltf -upAxis z`). The export browser shows the chosen type's options at its right, as the reference's does.
- **Materials and textures:** a physically based standard surface, and lambert, blinn, phong, phongE and surfaceShader with file textures and place2dTexture, assigned to objects or faces; Textured mode (6), anti-aliasing and ambient occlusion in the viewport; the Hypershade (Browser, Create panel, node graph, Material Viewer, Property Editor); a map button on every colour and number a texture can drive; the image behind the UV Editor.

  ![Materials and a file texture in Textured mode](docs/materials.png)
- **Lights:** ambient, directional, point, spot, area and volume lights with their icons, attributes and `.ma` layout; Use All Lights (7), Use Selected Lights, flat and no lights in the viewport, with decay, spot cones, blinn and phong highlights and light linking. A matte surface lights the same in the viewport and in Harold.
- **Two viewport renderers:** the Renderer menu of a view chooses Default, which every view starts in, or Legacy, the
  reference's Viewport 2.0 material shader. Default is OpenPBR shading from the scene's `openPBRSurface` and standard surface materials (base, metalness,
  diffuse and specular roughness, specular IOR, coat, transmission, emission) with a shadow map for a directional or
  spot light, six-face cube shadows for a point, area or volume light, screen-space contact shadows, ambient occlusion,
  and image-based lighting from an `aiSkyDomeLight`'s latlong image or colour. Default reads the same six standard
  lights and Harold's `aiAreaLight`, `aiMeshLight` and `aiPhotometricLight`; it falls back to Legacy on a device
  with less than 256 bytes of push constants. The viewport toolbar's AO button turns ambient occlusion on in either
  renderer, with more taps and a wider blur in Default.

**Pipeline**

- **Files:** `.ma` read and written in the standard layout, `.mb` read and written, deformer chains, constraints, utility nodes, expressions, materials,
  NURBS and references with their edits included; namespaces, instances, locked nodes, Z up.

  ![A character rig opened from a .ma file](docs/rig-file.png)
- **USD:** `.usd`, `.usda`, `.usdc` and `.usdz` read and written with transforms, meshes, skeletons, blend shapes, materials and animation; a stage in the scene with no node per prim, prims to select and edit, variants, payloads, a Layer Editor, editing a prim as scene nodes and merging back, `pxr` in Python. No C++ library needed. See [docs/USD.md](docs/USD.md).

**Scripting**

- **Scripting:** MEL (procedures, arrays, control flow, backticks), Python, the Python API OpenAztek, 2.0 and 1.0
  (`aztek.api.OpenAztek`, `aztek.OpenAztek`, `MScriptUtil` and all; the same modules as `maya.api.OpenMaya` and `maya.OpenMaya`), Python plug-ins of either API with `MPxCommand` and `MPxNode`; `scriptJob`, `scriptNode` and `evalDeferred`.
  The global MEL procedures scripts expect to find (`performDuplicate`, `FileMenu_SaveItem`, `toggleUIComponentVisibility`
  and the rest) come with Aztek in [scripts/](scripts/README.md), each written from a description of what the
  reference's does; 47 files so far, with what they still need in [docs/SCRIPT_GAPS.md](docs/SCRIPT_GAPS.md).

  ![A MEL script in the Script Editor building cubes with expressions, then playing them](docs/scripting.gif)
- **Script-built interface:** `window`, layouts, controls, menus and dialogs from MEL or Python become Qt widgets;
  attribute-bound controls, `AE<type>Template` procedures, `workspaceControl` docking, editors and panels created by script.

  ![A window built from MEL: sliders, a colour slider and buttons driving a procedure](docs/script-window.png)
- **C++ plug-ins:** a kit to rebuild a plug-in from its source against Aztek's headers. Commands, nodes and deformers load; more classes are coming.
- **Plug-in Manager and user folder:** load, unload and auto load plug-ins from one window. Preferences > User Folder points Aztek at an
  existing scripts, plug-ins, modules and prefs folder where it is, copies it into `~/aztek`, or uses a custom folder; `userSetup` scripts,
  `.mod` modules and the environment file are read. A compiled plug-in not built with the kit is listed and marked instead of failing.
  See [docs/PLUGINS.md](docs/PLUGINS.md).

**Interface**

- **Interface:** menu sets, shelves with an editor, hotkey editor, marking menus, hotbox, channel box, outliner, attribute
  editor, node editor, tool settings, panel layouts, workspaces. The Render View, Hypershade, UV Editor and Node Editor
  open as floating panels with the system's window border; dragged by their title onto the main window or another panel,
  they dock there, beside it or as a tab.
- **Units and up axis:** Y or Z up throughout (cameras, views, grid, primitives, files), and working units in every
  linear, angular and time unit, set in Preferences > Settings, with new scene defaults in File > New Scene's options.

![Aztek](docs/screenshot.png)

Lights in the viewport, Use All Lights on the 7 key, with highlights from blinn and phong materials:

![Lights](docs/lights.png)

Real production rigs, opened from their `.ma` files with their textures and control colours:

![Four production rigs](docs/rigs.png)

The Harold image above shows Toph and Lou, rendered with a sky and a key light added.

Left to right: a Venom rig, whose download names no author; Toph, a fan art rig by Ninjin Studios of the character from Nickelodeon's The Last Airbender; Lou, model by Dmitry Danilov and rig by Boris Klimov; the Waitress, model by François Boquet. They're used here as test rigs under their authors' terms and aren't part of this repository.

## In the works

- **Bicurious:** liquids, smoke, fire and procedural graphs inside the same solver as cloth, particles and rigid bodies,
  colliding with and pushing them.
- **Harold:** render layers and render setup, hair and curves, USD stages in renders.
- **Dynamics speed:** large cloth with self collision at interactive rates.
- **C++ plug-in kit:** commands, nodes and deformers load; more classes are coming.

## Examples

File > Examples opens over fifty scenes from the `examples` folder, a submenu per area, and the example scripts in the
Script Editor. Every one opens without a warning; a test plays each. Every copy of Aztek has them: the packages carry
the folder, and the program carries its own copy, which it writes to your data folder when no folder came with it.

![Twelve of the example scenes in the viewport](docs/examples.jpg)

![File > Examples > Modelling](docs/examples-menu.png)

- **Modelling:** every polygon primitive; a castle tower from one cylinder (extrude, inset, bevel, all as history);
  booleans; edge loops, a bridge and welds; a spaceship half mirrored live; smooth mesh preview with creases;
  duplicates and instances; NURBS curves and surfaces (revolve, loft, extrude along a path, planar trim); construction history.
- **UVs:** planar, cylindrical, spherical and automatic projections; unfold and layout, with a checker.
- **Rigging:** every constraint; an IK leg with a reverse foot; a spline IK tail; nonlinear deformers; lattice, wire,
  wrap, cluster and softMod; linear against dual quaternion skinning; blend shapes with an in-between; set driven keys;
  delta mush and jiggle; a small keyed arm rig.
- **Characters:** a keyed walking robot.
- **Animation:** a bouncing ball, a motion path, animation layers, expressions, pendulums on cycles.
- **Dynamics:** a flag in the wind, cloth on a moving collider, cloth draped on a ball, boxes on a hammock, bouncing
  particles, a particle vortex, particles with fields, a tower knocked down.
- **Rendering:** the standard surface's lobes, depth of field, motion blur, displacement and subdivision, every light
  type, the Cornell box, materials and lights.
- **Pipeline:** display layers and drawing overrides, namespaces and sets, a USD round trip, nested references.
- **Scripts:** a spiral staircase, scattered rocks, a rename window in MEL, a selection scriptJob, a mesh report with
  the API, a Python plug-in command, a turntable playblast.

`examples/tools/make_showcase.py` and `make_examples.mel` write the scenes.

## Screens

Four views, Space to toggle:

![Four views](docs/four-view.png)

Component editing with construction history (`polyExtrudeFace1` in the channel box's inputs) and the move manipulator:

![Modelling](docs/modelling.png)

Marking menus on the right button:

![Marking menu](docs/marking-menu.png)

Soft selection (B), falloff shown on the vertices:

![Soft selection](docs/soft-select.png)

UV Editor and its UV Toolkit, a crate unfolded and laid out:

![UV Editor](docs/uv-editor.png)

Graph Editor, with keyed channels in the channel box and key ticks on the time slider:

![Graph Editor](docs/graph-editor.png)

The production rig in another pose, its controls driving joints, skin and blend shapes:

![Production rig posed](docs/rig-pose.png)

A skinned arm with an RP IK handle and its controls:

![Rigging](docs/rigging.png)

Paint Skin Weights, with the influence list and brush settings in Tool Settings:

![Paint weights](docs/paint-weights.png)

NURBS surfaces from curves: a revolved vase, a loft, an extrusion along a path and a trimmed plane:

![NURBS](docs/nurbs.png)

Quad Draw on a live surface:

![Quad Draw](docs/quad-draw.png)

Dope Sheet, with the walking robot's keys:

![Dope Sheet](docs/dope-sheet.png)

Node Editor:

![Node Editor](docs/node-editor.png)

## Themes

The reference has one look, and Classic is it, the default. There are 26 built-in themes. The **Themes** box at the top right of the menu bar
changes the interface's colours, fonts and the shapes of its buttons, fields, tabs, arrows and scroll bars, live, and
Aztek keeps the choice. They are listed from darkest to lightest, and the icons turn dark on the light ones. Keating,
Novacek, Springer and Buxton are the Dracula, Nord and Solarized colour schemes as their authors publish them; Shaq
follows Unreal Engine 5's editor, DonnieAlex Blender's, Martino Softimage XSI and Macintosh macOS. The timeline and the drawn editors (Graph,
Node and UV Editor, marking menus) keep their own colours.

**Random!**, at the bottom of the box, makes a theme with every option at random, fonts included, and applies it: pick
it again for another. It is kept as your Random theme until the next roll, so one you like can be opened in the editor
and saved under a name; the editor's **Randomize** button does the same to the theme being edited.

**Theme Editor...**, at the bottom of the box, edits every part of a theme: each colour, the font and its size, italic
or bold menus and tabs, the fill (classic, flat or gradient), bevels (none, single like Motif, double like 1998), corner
radius, border width, padding, the tab, arrow and scroll bar styles, the buttons (soft, glossy, outline, inset, chunky,
underlined or pills), the edges round panels (line, groove, raised, thick, rounded or a drop shadow) and the panel title
bars (gradient, underlined, a highlight stripe, inverse or a folder tab). Every built-in theme has its own mix of these;
Martino follows Softimage XSI: warm grey panels, soft rounded buttons, blue-grey section bars, dark rounded entry
fields and a white bar for what is picked. Built-in themes stay as they are and saving makes a
copy of your own, kept as JSON in the Aztek settings folder's `themes` directory, which Import and Export pass around.
On Linux that folder is `~/.config/aztek` by default, and Aztek copies existing settings there without replacing files already present.

![Theme Editor](docs/theme-editor.png)

### Dark

| | | |
|---|---|---|
| ![Classic](docs/theme-classic.jpg) | ![Brinsmead](docs/theme-obsidian.jpg) | ![Nicolas](docs/theme-violet.jpg) |
| **Classic**: the reference's look, and the default | **Brinsmead**: pure black and electric cyan, monospaced type, square corners, cyan section bars | **Nicolas**: near-black with bold violet highlights and gold section bars |
| ![Flav](docs/theme-carbon.jpg) | ![Kurtenbach](docs/theme-tangerine.jpg) | ![Kovacs](docs/theme-midnight.jpg) |
| **Flav**: blue-grey charcoal and teal, round corners, pill tabs | **Kurtenbach**: charcoal and tangerine, geometric type, bold menus, borderless pills with big round corners | **Kovacs**: deep navy and gold, serif type with italic menus, round-topped tabs |
| ![Gibson](docs/theme-graphite.jpg) | ![Shaq](docs/theme-shaq.jpg) | ![Springer](docs/theme-solarized-dark.jpg) |
| **Gibson**: forest green and lime, humanist type, soft gradients | **Shaq**: after Unreal Engine 5's editor: near-black and charcoal panels, black fields, its bright blue, Roboto | **Springer**: Solarized dark with its yellow and orange: condensed type, single bevels, square edges |
| ![Ozymok](docs/theme-slate.jpg) | ![Keating](docs/theme-dracula.jpg) | ![DonnieAlex](docs/theme-classic98.jpg) |
| **Ozymok**: plum and hot pink, Gill-style type with italic menus, gradient buttons with a single bevel | **Keating**: the Dracula scheme: pink highlights, purple section bars, rounded type, very round corners | **DonnieAlex**: after Blender, in DonnieAlex's high-contrast take: mid-dark grey regions, its blue for panel headers and selection, grey rounded widgets, serif type |
| ![Novacek](docs/theme-nord.jpg) | ![Alias](docs/theme-alias.jpg) | ![Tureski](docs/theme-pewter.jpg) |
| **Novacek**: the Nord scheme: frost blues, geometric type, borderless, round-topped tabs | **Alias**: the late-90s animation package's dark scheme: slate panels, white section bars, fixed-width type | **Tureski**: warm pewter grey and orange, wide Verdana-style type, double bevels, orange section bars |

### Light

| | | |
|---|---|---|
| ![Sebdvvv](docs/theme-studio96.jpg) | ![Martino](docs/theme-martino.jpg) | ![Krieger](docs/theme-desktop98.jpg) |
| **Sebdvvv**: the mid-90s workstation animation packages: pink fields, bold italic menus, bevels | **Martino**: after Softimage XSI: warm grey, soft rounded buttons, blue-grey section bars, dark rounded fields, white selection | **Krieger**: silver and navy, two-tone bevels, 1998 |
| ![Wavefront](docs/theme-workstation.jpg) | ![Hourihan](docs/theme-poweranimator.jpg) | ![Hourcade](docs/theme-sky.jpg) |
| **Wavefront**: the light grey workstation desktop the reference first ran on | **Hourihan**: light grey with mid grey fields and darker section bars, round-topped tabs | **Hourcade**: pale sky blue and navy, humanist type, navy section bars, round pills |
| ![Buxton](docs/theme-solarized-light.jpg) | ![Hall](docs/theme-paper.jpg) | ![AshT](docs/theme-ash.jpg) |
| **Buxton**: Solarized light with its cyan and magenta: serif type, round-topped tabs | **Hall**: cream paper and red ink, typewriter type, bold menus, square corners, black section bars | **AshT**: after Hello Kitty: candy pink and white, red highlights and section bars, black outlines, rounded type |
| ![Macintosh](docs/theme-macos.jpg) | ![Stam](docs/theme-lilac.jpg) |  |
| **Macintosh**: after macOS: pale grey windows, white controls and fields, its blue, SF Pro, soft round corners, segmented tabs | **Stam**: mint and teal, rounded type, gradients, the roundest corners |  |

## Install

Packages are on the [releases page](https://github.com/srlegrand/Aztek/releases): each build of the main branch is a release of its own, numbered 2.N where N counts the changes so far. A build from source of the same commit shows the same number in Help > About. Each package goes out on its own: one whose build or check failed is left out of that release and the rest are published, and the website links every download to the newest release that has it.

**Updates.** On its first start Aztek asks whether to check for new versions when it starts; the answer can be changed in
Preferences > Interface, and Help > Check for Updates works either way. The check asks GitHub for the latest release and
sends nothing else: no identifiers, nothing about you or your scenes. That is the only request Aztek makes by itself. When
a newer version is out it shows what changed and offers to install it the way this copy was installed: the Windows
installer runs, a `.deb` or `.rpm` installs with your password, the macOS app, the AppImage and the Linux tarball are replaced in
place, and a Windows zip opens its download. Downloads are checked against the release's checksum. A copy built from
source is pointed at `git pull`. It goes through the proxy in `HTTPS_PROXY`, minding `NO_PROXY`.

| System | Package | Install |
| --- | --- | --- |
| Rocky Linux 8 | `aztek-<version>-1.el8.x86_64.rpm` | `sudo dnf install ./aztek-*.el8.x86_64.rpm` |
| Rocky Linux 9 | `aztek-<version>-1.el9.x86_64.rpm` | `sudo dnf install epel-release`, then `sudo dnf install ./aztek-*.el9.x86_64.rpm` |
| Ubuntu 24.04, 26.04 | `aztek_<version>-1_ubuntu24.04_amd64.deb`, `..._ubuntu26.04_...` | `sudo apt install ./aztek_*.deb` |
| Any other Linux, x86_64 | `aztek-<version>-x86_64.AppImage` | `chmod +x aztek-*.AppImage`, then run it (Qt, Python and PySide6 are inside) |
| Linux on 64-bit ARM, glibc 2.34+ (early) | `aztek-<version>-aarch64.AppImage` | `chmod +x aztek-*.AppImage`, then run it. Needs a Vulkan driver (Mesa's on a Raspberry Pi 5, Asahi, Snapdragon laptops) |
| Windows 10 and 11 | `aztek-<version>-setup-x64.exe`, or the zip | Run it. It isn't signed, so SmartScreen asks first: see [Running unsigned builds](#running-unsigned-builds). Its "Add Aztek to PATH" puts the install's `bin` on PATH, so `aztek`, `aztekpy` and `harold` work from any prompt |
| macOS 14+, Apple silicon (early) | `aztek-macos-arm64.zip` | Unzip and open Aztek.app. It isn't notarized, so macOS blocks it the first time: see [Running unsigned builds](#running-unsigned-builds) |

Every package carries Python 3.13 and Qt and PySide6/shiboken6 6.8.3, the VFX Reference Platform CY2027's versions.
None needs Qt or Python installed. The ARM AppImage is the other exception: it uses
Rocky 9's Qt and has no PySide6, as there is no arm64 Linux kit or wheel.

The viewport needs a Vulkan driver: NVIDIA's own driver has one, and Mesa's `mesa-vulkan-drivers` covers AMD and Intel. On macOS it
draws through MoltenVK, which comes inside Aztek.app.
On Windows, if Aztek says it cannot find a Vulkan driver, install the latest driver for your GPU from your laptop or GPU vendor.
If the GPU has no Vulkan support, the viewport stays blank even when the scene contains objects.

[docs/PACKAGING.md](docs/PACKAGING.md) has the details, and how the packages are built.

### Running unsigned builds

The Windows and macOS packages aren't signed with a paid certificate, so the system asks before running them the first
time. Each new download asks again. Builds made from source are never asked about.

**Windows.** The installer and `aztek.exe` show "Windows protected your PC": click **More info**, then **Run anyway**.
For the zip, unblock it before unzipping, so the files inside don't each ask: right-click the zip, Properties, tick
**Unblock**, OK. Or in PowerShell:

    Unblock-File .\aztek-windows-x86_64.zip

Already unzipped, from inside the folder it went to:

    Get-ChildItem -Recurse | Unblock-File

**macOS.** Opening Aztek.app says it can't be checked. Either, once after unzipping:

    xattr -dr com.apple.quarantine ~/Downloads/Aztek/Aztek.app

or open it once, close the warning, go to System Settings > Privacy & Security, scroll down and click **Open Anyway**.
Since macOS 15, right-click and Open no longer gets past it. To stop the check for every app on the Mac,
`sudo spctl --global-disable`, then choose Anywhere under Privacy & Security; `sudo spctl --global-enable` turns it back on.

## Hotkeys

| Keys | Action |
|---|---|
| Alt + left / middle / right drag, wheel | Tumble, track, dolly |
| Space tap / hold | Toggle single and four views / hotbox |
| Right button | Marking menu (Shift: modelling tools, Ctrl: selection conversion) |
| Q W E R | Select, move, rotate, scale |
| D or Insert | Edit Pivot on or off; hold D to edit the pivot only while held |
| F / A, Shift+F / Shift+A | Frame selection / all, in every view |
| F8, F9, F10, F11 | Object/component, vertex, edge, face |
| F2, F3, F4 | Animation, Modeling, Rigging menu sets |
| Click, drag, double-click an edge | Select, marquee, edge loop |
| Shift / Ctrl / Ctrl+Shift click | Toggle / deselect / add |
| Z, Shift+Z (Ctrl+Z, Ctrl+Y) | Undo, redo |
| G | Repeat last |
| Ctrl+D, Shift+D, Ctrl+G, P, Shift+P | Duplicate, duplicate with transform, group, parent, unparent |
| Ctrl+E, Ctrl+B | Extrude, bevel |
| 1, 2, 3 | Smooth preview off, cage, smooth |
| 4, 5 | Wireframe, shaded |
| B | Soft selection |
| > and < | Grow and shrink selection |
| Delete | Delete objects or components |
| S, Shift+W/E/R | Set key on all channels, on translate/rotate/scale |
| . and , | Next and previous key |
| Alt+. and Alt+, | Next and previous frame |
| Alt+V, Esc | Play / stop |
| Ctrl+A | Attribute Editor |
| Enter | Joint tool: end the chain and go back to Select. Curve, Multi-Cut and polygon tools: finish |
| B drag | Paint and sculpting tools: size the brush |
| Ctrl+Shift+T | Tool Settings (also: double-click a tool) |

In the channel box, click channel names to pick them (Ctrl and Shift add); middle-drag in a view then changes them
(Ctrl fine, Shift coarse). Right-click for keys, locks and connections.

## Editors

- Outliner, Channel Box / Layer Editor, Attribute Editor (docked), Script Editor with MEL and Python tabs and Ctrl+F
  to find. The thin arrows either side of the views fold away everything docked on that side; the lock beside the
  Themes box keeps docked panels where they are.
- Welcome screen at start-up (Help > Welcome Screen): a new or recent scene, the examples, links, every change of the day
  in its commit's words and the newest announcements when update checks are on, a parity bar along the foot (the parity ledger's
  overall figure, over the reference's commands, flags and global MEL procedures so far, `aztekParity -q`: 41.7% on
  21/11/2026), and a greyhound under the links. Help > Thanks lists the GitHub names the welcome screen thanks, each a
  link to its page.

  ![Welcome screen](docs/welcome-screen.png)
- Modeling Toolkit (Windows > Modeling Editors), docked beside the Attribute Editor: selection types (Ctrl converts,
  Ctrl+Shift to the perimeter, right-click clears), how clicks select, symmetry and constraints, soft selection, and
  the mesh, component and tool buttons.

  ![Modeling Toolkit](docs/modeling-toolkit.png)
- UV Toolkit, the UV Editor's side panel, and its menu bar (Edit, Create, Select, Cut/Sew, Modify, Tools, View, Image,
  UV Sets): selection, pinning, select by type (unmapped faces too), soft selection, transform, texel density,
  projections (camera and normal based, best plane), cut and sew (Create UV Shell, Create Shell (Grid)), unfold and
  unfold along U or V, straighten UVs, borders and shells, align and snap (snap and stack, normalize along U or V),
  arranging shells (orient, stack, unstack, gather, distribute toward a target, randomize, layout along U or V),
  measuring, Cycle, Symmetrize, Unitize and Map Border, the Transform section's pivot (on the selection, the UV area or
  put with Edit Pivot), step snaps, Retain component spacing and Prevent Negative Scale, the Lattice tool, Warp Image, and the
  brushes (Grab, Pinch, Smear, Pin, Optimize, Unfold, Sew; B and drag sizes them).

  ![UV Toolkit](docs/uv-toolkit.png)
- Graph Editor and Dope Sheet (Windows > Animation Editors): curves and keys of the selection.
- Node Editor (Windows > General Editors): graph the selection, wire attributes, Tab creates a node.
- Hotkeys and shelves are editable; the Hotkey Editor is under Windows > Settings/Preferences.
- UV Editor (Windows > Modeling Editors, or UV > UV Editor): UVs by UV, shell, edge or face; move, rotate and scale;
  projections, unfold, layout, cut, sew, normalize, flip.

## Scripting

The command line and the Script Editor take MEL (`polyCube -w 2 -sx 3; select -r pCube1.f[0:3]; polyExtrudeFacet
-ltz 1;`) and Python:

    from aztek import cmds
    cube = cmds.polyCube(w=2)[0]
    cmds.setKeyframe(cube, at='ty', t=1, v=0)
    cmds.setKeyframe(cube, at='ty', t=24, v=5)
    cmds.polyCube(cube, q=True, w=True)

`aztek.cmds` is the module of record; the compatibility modules map onto it, so existing scripts run unchanged. Python plug-ins load
with `loadPlugin`. C++ plug-ins rebuilt with the kit in `devkit/` load too ([docs/PLUGINS.md](docs/PLUGINS.md)).

## Compatibility

Existing scripts, habits and `.ma` files carry over: a production rig opens, poses and saves (see "A real rig" in
[PARITY.md](PARITY.md)). Behaviour comes from published references, so expect differences in details; PARITY.md lists
the known ones. `.mb` files open, import, load as references and save; a few things are left out of a saved `.mb` with a warning (see PARITY.md), and the C++ plug-in kit covers part of the API so far. Materials and render settings from other renderers in a file are
read and rendered with Harold, with a warning naming what isn't honoured.

## Discord

Questions, show and tell, and talk about where Aztek goes next: [join the Discord](https://discord.gg/XDEgpWDVKa).

## Licence

[MIT-0](LICENSE): use, copy, change, sell, re-host and redistribute it however you like, with no attribution required.
One table of binary file type ids in it comes from an MIT project; its notice is in [THIRD_PARTY.md](THIRD_PARTY.md).
Copies on any other host are welcome (see [Copy it anywhere](#copy-it-anywhere)). It comes with no warranty of any kind.
