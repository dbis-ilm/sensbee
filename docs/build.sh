#!/bin/bash

# Clear build folder
rm -r ./build

# Build the docs
docker run --rm -v ./:/docs sphinxdoc/sphinx bash -c 'pip install furo sphinx-markdown-builder sphinx-copybutton && make html && make markdown'
