#!/bin/bash

find . -name '*.rs' | while read f; do
  echo "# $f"
  rg '^(pub |impl|fn |struct |enum |trait |mod )' "$f"
  echo
done > project_outline.txt
