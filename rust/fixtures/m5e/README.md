# M5-E ChatML fixture

`qwen-7b-chatml.jinja` 固定 90 主机 Qwen-7B 的原生 ChatML 语义，用于 M5-E
单 GPU Lab smoke。模型自身的 `qwen_generation_utils.py::make_context` 会始终先生成
system 块；未提供 system 消息时，其内容为空。随后按消息顺序生成 user/assistant 块，
并在需要生成回复时追加 assistant 起始块。

- 文件为 UTF-8、单行、LF 结尾，文件 SHA-256 为
  `0d06c62751b46ff03bde0e08ae4d351f056e95885b3c258382492686965af15d`。
- 删除唯一结尾 LF 后的有效模板长度为 381 bytes，SHA-256 为
  `74c5628e34e1d05c43a639966d7f5861d01cfd89ebafa39b3976f4d6c12dfd14`。
- 正式命令必须把有效模板作为一个
  `--vllm-extra-arg=--chat-template=<template>` 参数传入；不得使用 `eval`，也不得把
  仓库或模型目录改成可写后临时生成模板文件。

该 fixture 只覆盖当前 M5-E 的 string content、system/user/assistant 消息和
`add_generation_prompt`。它不声明 tool calling、多模态消息或其他 Qwen 版本兼容性。
