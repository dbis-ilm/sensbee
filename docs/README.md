## Project Documentation

This directory contains the Sphinx-generated documentation for the project.

## Viewing the Documentation

To view a local build of the documentation either open the generated HTML [index.html](build/html/index.html) file or 
run the appropriate command for your operating systems from the project root directory.

For Linux

```sh
xdg-open docs/build/html/index.html
```

For macOS

```sh
open docs/build/html/index.html
```

## Rebuilding the Documentation

To recreate or update the documentation, use the provided build script:

```sh
sh build.sh
```

This script will regenerate all necessary documentation files.

Please note, for Windows the specified mount path must be adapted manually to point to the root directory of the docs folder:
```docker run --rm -v "C:\...\SensBee\docs:/docs" ...```