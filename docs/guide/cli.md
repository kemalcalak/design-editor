# Command line

`de` drives a running editor, or a headless one, over MCP:

```bash
de start --file landing.de          # open the editor on a file
de tools                            # list every MCP tool with its schema
de <tool> key=value ...             # call one tool
de export-frames --output-dir out   # export every frame on the active page
de export-deck --output deck.pptx   # export the page's boards as a deck
de start --headless --file a.de     # windowless MCP server for CI or scripts
```

`de help` lists every command.
