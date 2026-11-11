#!/usr/bin/env python3
"""岛仔状态总线本地调试台（零依赖）。

    py status_console.py
    浏览器打开 http://127.0.0.1:8765

只监听 127.0.0.1，不对外暴露；所有写入都走 StatusBus 的文件锁，
和 send_status.py / examples.py 并发使用不会互相覆盖。
"""
import json
import http.server
from send_status import StatusBus

PORT = 8765
bus = StatusBus()

HTML = """<!doctype html>
<html lang="zh-CN"><head><meta charset="utf-8">
<title>岛仔状态总线调试台</title>
<style>
body{font-family:system-ui,-apple-system,"Microsoft YaHei",sans-serif;background:#f7f7f5;color:#2c2c2a;margin:0;padding:32px}
.wrap{max-width:720px;margin:0 auto}
h1{font-size:18px;font-weight:500;margin:0 0 6px}
p.hint{font-size:13px;color:#5f5e5a;margin:0 0 24px;line-height:1.6}
.card{background:#fff;border:1px solid #e3e1da;border-radius:12px;padding:20px;margin-bottom:16px}
label{display:block;font-size:12px;color:#5f5e5a;margin:0 0 4px}
input,select{width:100%;box-sizing:border-box;padding:8px 10px;border:1px solid #d3d1c7;border-radius:8px;font-size:13px;background:#fff;color:#2c2c2a;margin-bottom:12px}
.row{display:grid;grid-template-columns:1fr 1fr;gap:12px}
button{padding:8px 16px;border:1px solid #b4b2a9;border-radius:8px;background:#fff;color:#2c2c2a;font-size:13px;cursor:pointer;margin-right:8px}
button.primary{background:#185fa5;border-color:#185fa5;color:#fff}
button:hover{border-color:#185fa5}
pre{background:#f1efe8;border-radius:8px;padding:12px;font-size:12px;overflow:auto;max-height:260px;margin:0}
.badge{display:inline-block;font-size:12px;background:#e1f5ee;color:#0f6e56;border-radius:6px;padding:2px 8px;margin-bottom:12px}
</style></head><body><div class="wrap">
<h1>岛仔状态总线调试台</h1>
<p class="hint">写一条状态，2 秒内出现在灵动岛；同 id 再点「上岛」就是覆写刷新，点「撤下」立即消失。<br>文件位置：%APPDATA%\\Daozai\\island_status.json</p>
<div class="card">
<div class="badge" id="msg">就绪</div>
<div class="row">
<div><label>id（覆写用的 key）</label><input id="id" value="demo"></div>
<div><label>text 标题</label><input id="text" value="构建中 45%"></div>
</div>
<label>detail 副标题</label><input id="detail" value="cargo build --release">
<div class="row">
<div><label>progress 进度 0~1（留空不画进度条）</label><input id="progress" value="0.45" type="number" step="0.01" min="0" max="1"></div>
<div><label>color 状态色</label><select id="color"><option value="">默认灰</option><option value="blue" selected>blue</option><option value="green">green</option><option value="amber">amber</option><option value="red">red</option><option value="gray">gray</option></select></div>
</div>
<label>click 动作（copy:文本 / url:https://... / file:路径，留空无按钮）</label><input id="click" value="copy:cargo build --release">
<div class="row">
<div><label>seconds（0 = 默认 8 秒，靠覆写续期）</label><input id="seconds" value="0" type="number"></div>
</div>
<button class="primary" onclick="send('set')">上岛 / 覆写</button>
<button onclick="send('remove')">撤下这个 id</button>
<button onclick="send('clear')">清空全部</button>
</div>
<div class="card"><label>当前 island_status.json</label><pre id="current">加载中…</pre></div>
</div>
<script>
async function refresh(){
  const r=await fetch('/api/status'); const j=await r.json();
  document.getElementById('current').textContent=JSON.stringify(j,null,2);
}
async function send(op){
  const v=id=>document.getElementById(id).value.trim();
  let body={id:v('id')};
  if(op==='set'){
    const p=v('progress');
    body={id:v('id'),text:v('text'),detail:v('detail'),seconds:Number(v('seconds')||0),color:v('color'),click:v('click')};
    if(p!=='')body.progress=Number(p);
  }
  const r=await fetch('/api/'+op,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(body)});
  const j=await r.json();
  document.getElementById('msg').textContent=j.ok?('已'+({set:'上岛',remove:'撤下',clear:'清空'}[op])):('失败：'+j.error);
  refresh();
}
refresh(); setInterval(refresh,3000);
</script></body></html>"""


class Handler(http.server.BaseHTTPRequestHandler):
    def _reply(self, code, body, ctype):
        raw = body.encode("utf-8")
        self.send_response(code)
        self.send_header("Content-Type", ctype + "; charset=utf-8")
        self.send_header("Content-Length", str(len(raw)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        self.wfile.write(raw)

    def do_GET(self):
        if self.path.startswith("/api/status"):
            self._reply(200, json.dumps(bus.list(), ensure_ascii=False), "application/json")
        elif self.path.startswith("/"):
            self._reply(200, HTML, "text/html")
        else:
            self._reply(404, '{"ok":false,"error":"not found"}', "application/json")

    def do_POST(self):
        try:
            size = int(self.headers.get("Content-Length", 0))
            payload = json.loads(self.rfile.read(size) or b"{}")
            if self.path == "/api/set":
                bus.set(payload)
            elif self.path == "/api/remove":
                bus.remove(str(payload.get("id", "")))
            elif self.path == "/api/clear":
                bus.clear()
            else:
                return self._reply(404, '{"ok":false,"error":"not found"}', "application/json")
            self._reply(200, '{"ok":true}', "application/json")
        except Exception as exc:
            self._reply(400, json.dumps({"ok": False, "error": str(exc)}, ensure_ascii=False), "application/json")

    def log_message(self, *args):
        pass


if __name__ == "__main__":
    print("岛仔状态总线调试台：http://127.0.0.1:{}".format(PORT))
    print("Ctrl+C 退出；只监听本机回环地址")
    http.server.ThreadingHTTPServer(("127.0.0.1", PORT), Handler).serve_forever()
