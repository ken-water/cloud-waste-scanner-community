# Skills 使用说明

这份说明用于统一 Cloud Waste Scanner skills 的中文对外表述。

## 一句话说明

Cloud Waste Scanner 的 skills 不一定要求先安装 app，但连接本地 app 和本地 API 时效果最好。

如果没有安装 app，也可以基于导出的 JSON、CSV、TXT 等证据文件使用 skills，只是能力会弱一些。

## 两种运行方式

### 1. 连接模式

连接模式指：

- 已安装 Cloud Waste Scanner
- 已启用本地 API
- skill 直接读取本机上的扫描结果、报告和历史数据

这是效果最好的模式，因为：

- 数据更新
- 字段更完整
- 能利用扫描、历史、报告等完整上下文
- 不需要手工整理输入材料

### 2. 文件模式

文件模式指：

- 不要求 app 正在运行
- 直接使用导出的证据文件

常见输入：

- findings JSON
- findings CSV
- report JSON
- 从报告中复制出来的文本

这个模式适合：

- 先低门槛试用
- 分享给老板、财务、审计或其他评审人
- 在没有本机环境时先做解释和整理

限制也很明确：

- 数据可能不是最新
- 字段可能缺失
- 没法重新触发扫描
- 历史上下文可能不完整

## 没有数据时怎么办

如果既没有：

- 本地 app / 本地 API
- 也没有导出的证据文件

那么 skill 不应该假装自己还能给出可靠结果。

此时它只能：

- 解释方法
- 说明需要哪些输入
- 告诉用户如何获取可用证据

它不应代替 Cloud Waste Scanner 自己去扫描云资源，也不应索取云凭证。

## 当前 skill 能做什么

以 `cws-report-explainer` 为例，它主要负责：

- 解释 findings
- 排出本周优先处理项
- 生成面向老板、财务、执行人的不同摘要
- 将本地 API 或导出文件整理成更容易理解的结果

它不负责：

- 自动分配 owner
- 自动派单
- 自动升级或催办
- 审批流
- 企业级审计控制

## 社区版、Team、企业版的区别

### Community skills

负责：

- 解释
- 整理
- 准备

典型能力：

- finding explainer
- finance summary
- executive brief
- weekly operator brief

### Team skills

负责：

- 协调
- 执行
- 跟踪

典型能力：

- owner assignment assistant
- weekly governance pack
- manager review copilot

### Enterprise skills

负责：

- 控制
- 审计
- 合规

典型能力：

- audit evidence automation
- policy exception review
- compliance rollup

## 对创业团队的建议

如果你是一个 1 到 10 人左右的创业团队，建议这样使用：

1. 先安装 Community 版 app，跑出第一轮扫描结果。
2. 用 Community skill 把 findings 解释清楚。
3. 导出报告给老板或财务确认优先级。
4. 先通过人工方式推进处理。
5. 当你们开始出现明确的 owner、周例会、治理节奏，再考虑 Team 版能力。

## 对外统一说法

推荐统一使用以下表述：

`Cloud Waste Scanner skills do not always require the app, but they work best when connected to the CWS local API.`

中文版本：

`Cloud Waste Scanner 的 skills 不一定要求先安装 app，但连接 CWS 本地 API 时效果最好。`
