import sys
for path in [r'D:\work\cad\PID自研软件验证\I井场 注采阀组工艺及自控流程图.pid',
             r'D:\work\cad\PID自研软件验证\集注站 凝析油闪蒸单元 一级闪蒸分离器B工艺及自控流程图.pid']:
    data=open(path,'rb').read()
    pat='∽'.encode('utf-16-le')
    hits=[]
    i=0
    while True:
        i=data.find(pat,i)
        if i<0: break
        hits.append(i); i+=2
    print(path.split(chr(92))[-1],len(hits))
    for h in hits[:6]:
        # context: decode surrounding utf16 run
        s=max(0,h-80); seg=data[s:h+120]
        # printable utf16 chars
        txt=''.join(chr(seg[j]|(seg[j+1]<<8)) if 0x20<=seg[j]|(seg[j+1]<<8)<=0x9FFF else '.' for j in range(0,len(seg)-1,2))
        print(f'  @{h:06x} ...{txt}...')
