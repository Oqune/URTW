"""Render a read-only Ratatui --snapshot JSON using Pillow (development only)."""
import argparse
import json
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("snapshot", type=Path)
parser.add_argument("output", type=Path)
parser.add_argument("--font", default="C:/Windows/Fonts/consola.ttf")
args = parser.parse_args()
data = json.loads(args.snapshot.read_text(encoding="utf-8"))
font = ImageFont.truetype(args.font, 16)
cw, ch, pad = 10, 21, 14
image = Image.new("RGB", (data["width"]*cw+pad*2, data["height"]*ch+pad*2), "#020617")
draw = ImageDraw.Draw(image)
for index, cell in enumerate(data["cells"]):
    x, y = (index % data["width"])*cw+pad, (index // data["width"])*ch+pad
    draw.rectangle((x,y,x+cw-1,y+ch-1),fill=cell["bg"])
    if cell["text"].strip():
        draw.text((x,y+1),cell["text"],font=font,fill=cell["fg"])
args.output.parent.mkdir(parents=True,exist_ok=True)
image.save(args.output)
