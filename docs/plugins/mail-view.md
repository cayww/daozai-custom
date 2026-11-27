# 邮箱视图扩展（mail.view v1）

无脚本资源插件，启用后扩展原生邮箱页。未启用或卸载后恢复原来的邮箱页。只能启用一个邮箱视图。请在支持本能力的本体发布后再上架插件。

维护者负责清单、视图配置、设置项、封面和插件版本，不需要提交假邮件或脚本。邮件数据、收信、验证码、原生阅读器和本机／同步写操作由本体负责，插件不读取这些数据。

## plugin.json

```json
{
  "id": "author.mail-view",
  "name": "多账号邮箱视图",
  "author": "Author",
  "version": "1.0.0",
  "api": 1,
  "runtime": "assets",
  "entry": "mail-view.json",
  "cover": "cover.png",
  "owner": "mail",
  "capabilities": [
    "mail.view",
    "settings"
  ],
  "dataVersion": 1,
  "permissions": {},
  "settings": [
    {
      "key": "since",
      "label": "起始日期（0001-01-01 表示不限）",
      "type": "date",
      "default": "0001-01-01"
    },
    {
      "key": "nicknames",
      "label": "邮箱别名",
      "type": "list",
      "default": "[]",
      "maxItems": 20,
      "item": [
        {
          "key": "address",
          "label": "已连接的完整邮箱地址",
          "type": "text",
          "default": "请输入邮箱"
        },
        {
          "key": "alias",
          "label": "别名",
          "type": "text",
          "default": "请输入别名"
        }
      ]
    }
  ]
}
```

## mail-view.json

```json
{"version":1,"showUnread":true,"startDateSetting":"since","aliasesSetting":"nicknames"}
```

日期和别名设置可省略。日期按设备时区计算；已指定日期时只显示收信日期明确且符合范围的邮件。列表、未读数和回收站共用筛选范围。单账号且未按日期筛选时才启用批量已读；按日期筛选时请逐封操作。别名仅作为界面标签，不参与邮件和账号标识。

## 验收

用两个隔离的测试邮箱检查账号切换、同名别名、未读数、回收站、相同 UID 的不同邮件、扫码/验证码提醒及停用回退。分别验证仅本机与同步操作不触及其他账号。
