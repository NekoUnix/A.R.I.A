"""Generate original, editable ARIA PNG/GIF action examples (requires Pillow)."""
from pathlib import Path
import json
from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent
ART = ROOT / 'artwork'
ART.mkdir(exist_ok=True)

def puppet(talk=False, blink=False, smile=False):
    im=Image.new('RGBA',(512,512));d=ImageDraw.Draw(im)
    d.rounded_rectangle((113,330,399,504),radius=58,fill='#292d49')
    d.polygon([(154,189),(135,58),(229,127)],fill='#dadff2',outline='#141c35',width=10)
    d.polygon([(286,127),(377,58),(358,193)],fill='#dadff2',outline='#141c35',width=10)
    d.ellipse((123,113,389,381),fill='#dadff2',outline='#141c35',width=8)
    for x in [202,310]:
        if blink:d.arc((x-30,204,x+24,242),0,180,fill='#141c35',width=9)
        else:
            d.ellipse((x-27,205,x+22,264),fill='#141c35')
            d.ellipse((x-18,214,x+11,255),fill='#6be0ce')
            d.ellipse((x-12,217,x-2,230),fill='white')
    d.polygon([(245,276),(267,276),(256,286)],fill='#141c35')
    if talk:d.ellipse((236,300,276,340),fill='#141c35');d.arc((239,315,273,347),180,360,fill='#e8a1bd',width=10)
    elif smile:d.arc((229,289,283,328),0,180,fill='#141c35',width=6)
    else:d.line([(237,311),(256,321),(275,311)],fill='#141c35',width=6)
    d.polygon([(224,393),(288,393),(256,423)],fill='#6be0ce')
    return im

for name,options in [('idle',{}),('talking',{'talk':True}),('blink',{'blink':True})]:puppet(**options).save(ART/(name+'.png'))
frames=[puppet(talk=(n%2==0),smile=True) for n in range(8)]
frames[0].save(ART/'excited.gif',save_all=True,append_images=frames[1:],duration=[120,80]*4,loop=0,disposal=2)
# The SVG is an editable drawing template with named eye/mouth groups.
(ART/'avatar-template.svg').write_text('''<svg xmlns="http://www.w3.org/2000/svg" width="512" height="512" viewBox="0 0 512 512">
<rect x="113" y="330" width="286" height="174" rx="58" fill="#292d49"/>
<path d="M154 189L135 58L229 127M286 127L377 58L358 193" fill="#dadff2" stroke="#141c35" stroke-width="10"/>
<ellipse cx="256" cy="247" rx="133" ry="134" fill="#dadff2" stroke="#141c35" stroke-width="8"/>
<g id="eyes"><ellipse cx="202" cy="235" rx="27" ry="30" fill="#141c35"/><ellipse cx="310" cy="235" rx="27" ry="30" fill="#141c35"/></g>
<g id="mouth"><path d="M237 311L256 321L275 311" fill="none" stroke="#141c35" stroke-width="6"/></g>
<path d="M224 393L288 393L256 423Z" fill="#6be0ce"/></svg>''',encoding='utf-8',newline='\n')
def state(id,name,path,trigger,priority,motion='None'):
 return dict(id=id,name=name,path='artwork/'+path,enabled=True,trigger=trigger,priority=priority,
             fade_in=.12,fade_out=.16,minimum_hold=.1,transition='Crossfade',
             motion=dict(kind=motion,strength=.06,duration=.4,frequency=5,repeat=False),
             gif_speed=1,gif_loop=True,restart_gif=True)
states=[state(1,'Idle','idle.png','Idle',0),state(2,'Talking','talking.png','Talking',10,'Jump'),
        state(3,'Blink','blink.png','Blink',20),state(4,'Excited GIF','excited.gif','Manual',30,'Shake')]
(ROOT/'starter.aria-images.json').write_text(json.dumps(dict(enabled=True,states=states,manual=None),indent=2)+'\n',encoding='utf-8',newline='\n')
print('Generated PNG/GIF artwork, editable SVG and four action states')
