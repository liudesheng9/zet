//! End-to-end integration tests for Level 2 formal economic models (docs/formal-models-proposal.md):
//! - P2b-1 R6 notation lookup parity (`<head>|a|a` and 1-hop `沿用 [[...]]`) in `zt` gate AND `zt-check`
//! - `get_model` Layer 1 build report (`claim_objects`, `family_tags`, `role` on `build.order`,
//!   Side-branch `Derivation` chains breaking at `论证`, accurate `proof.steps` excluding `边界`/`例示`,
//!   badges, and Layer 1 faults enforced unconditionally on all Level 2 formal economic models)
//! - Mathematical units (`form: "theorem"`) trigger zero formal-model / `FM1`–`FM6` faults.

#![allow(dead_code)]

use assert_cmd::Command;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command as StdCommand, Stdio};
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

struct TestHome {
    _tmp: TempDir,
    config_dir: std::path::PathBuf,
    archive_root: std::path::PathBuf,
}

impl TestHome {
    fn new() -> Self {
        let tmp = tempfile::tempdir().expect("tempdir");
        let config_dir = tmp.path().join("config");
        let archive_root = tmp.path().join("archive");
        let home = Self {
            _tmp: tmp,
            config_dir,
            archive_root,
        };
        home.cmd()
            .args([
                "config",
                "set",
                "archive_root",
                home.archive_root.to_str().unwrap(),
            ])
            .assert()
            .success();
        home.cmd().arg("up").assert().success();
        home
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("zt").expect("zt binary");
        cmd.env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0");
        cmd
    }

    fn mcp(&self) -> Mcp {
        let mut child = StdCommand::new(assert_cmd::cargo::cargo_bin("zt"))
            .arg("mcp")
            .env("ZT_CONFIG_DIR", &self.config_dir)
            .env("ZT_GUI_PORT", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start zt mcp");
        let stdin = child.stdin.take().expect("mcp stdin");
        let stdout = BufReader::new(child.stdout.take().expect("mcp stdout"));
        let mut mcp = Mcp {
            child,
            stdin,
            stdout,
            next_id: 1,
        };
        mcp.request(
            "initialize",
            json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "zt-formal-model-tests", "version": "1" } }),
        );
        mcp.send(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }));
        mcp
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.archive_root.join("zt.pid"));
        thread::sleep(Duration::from_millis(80));
    }
}

struct Mcp {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: u64,
}

impl Mcp {
    fn send(&mut self, message: &Value) {
        writeln!(self.stdin, "{message}").expect("write MCP message");
        self.stdin.flush().expect("flush MCP message");
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("read MCP reply");
        serde_json::from_str(&line).expect("MCP reply is JSON")
    }

    fn call_raw(&mut self, name: &str, args: Value) -> Value {
        self.request("tools/call", json!({ "name": name, "arguments": args }))
    }

    fn call(&mut self, name: &str, args: Value) -> Value {
        let reply = self.call_raw(name, args);
        assert!(reply["error"].is_null(), "MCP error for {name}: {reply}");
        assert_ne!(
            reply["result"]["isError"],
            json!(true),
            "tool {name} returned error: {reply}"
        );
        let text = reply["result"]["content"][0]["text"]
            .as_str()
            .expect("tool text");
        serde_json::from_str(text).expect("tool JSON")
    }
}

impl Drop for Mcp {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn formal_model_end_to_end_r6_parity_chains_badges_unconditional_enforcement_and_math_unit_isolation() {
    let home = TestHome::new();
    let mut mcp = home.mcp();

    // Create literature tree.
    mcp.call(
        "create_literature",
        json!({
            "bibtex": "@article{arro2026flight, author={Arro and Cannarsa}, title={Flight to Convenience}, year={2026}}"
        }),
    );

    // 1. Create a Level 2 formal Model Card at arro2026flight/1|a.
    let s1 = mcp.call(
        "create_successor",
        json!({
            "at": "arro2026flight",
            "kind": "direct",
            "title": "第 1 节：基准模型",
            "body": "本节建立两国模型。"
        }),
    );
    let sec1 = s1["address"].as_str().unwrap().to_string();

    let head_body = "\
问题：便利收益外逃如何影响两国通胀与产出。

**目标对象与标签**
- 对象：market-equilibrium、dynamics-limit
- 标签：open-economy-dsge

**原语与判据**
两国三资产环境，求解采用竞争均衡判据。

**主要结果**
| 结果 | 逻辑力度 | 作用域 / 条件 | 依据状态 |
| --- | --- | --- | --- |
| 命题 1 | characterization | 稳态附近 | 局部证明 |";
    let head = mcp.call(
        "create_successor",
        json!({
            "at": sec1,
            "kind": "side",
            "type": "model",
            "level": "formal",
            "title": "两国便利收益外逃模型",
            "body": head_body
        }),
    );
    let head_addr = head["address"].as_str().unwrap().to_string();

    // Setting at <head>|a (NO embedded term table!).
    let setting = mcp.call(
        "create_successor",
        json!({
            "at": head_addr,
            "kind": "side",
            "facet": "设定",
            "title": "设定：两国三资产环境",
            "body": "两国各有一类家庭与三类债券。\n\n**起点**\n- 原始设定：基准框架。\n\n**设定**\n稳态便利收益份额记为 $\\nu$。\n\n**直觉**\n- 是什么：两国持有跨国安全资产。\n- 为什么需要：刻画便利收益传导。\n- 怎样进入模型：后续章节均基于此。"
        }),
    );
    let setting_addr = setting["address"].as_str().unwrap().to_string();
    assert_eq!(setting_addr, format!("{head_addr}|a"));

    // Notation card at <head>|a|a with **统一用语** (R6 forbidden term: 流动性溢价 -> 便利收益).
    let notation = mcp.call(
        "create_successor",
        json!({
            "at": setting_addr,
            "kind": "side",
            "facet": "符号表与读法",
            "title": "符号表与读法",
            "body": "**怎样读卡片上的记号**\n下标标注国别。\n\n| 符号 | 含义 | 首次出现 |\n| --- | --- | --- |\n| $\\nu$ | 稳态便利收益份额 | [[arro2026flight/1|a|a]] |\n| $x$ | 产出缺口 | [[arro2026flight/1|a|a]] |\n| $\\pi$ | 通胀 | [[arro2026flight/1|a|a]] |\n\n**统一用语**\n\n| 术语 | 不用 |\n| --- | --- |\n| 便利收益 | 流动性溢价 |"
        }),
    );
    let notation_addr = notation["address"].as_str().unwrap().to_string();
    assert_eq!(notation_addr, format!("{setting_addr}|a"));

    // Criterion under setting (<head>|a|b).
    let crit = mcp.call(
        "create_successor",
        json!({
            "at": setting_addr,
            "kind": "side",
            "facet": "均衡定义：两国竞争均衡",
            "title": "均衡定义：两国竞争均衡",
            "body": "定义两国资产市场与商品市场同时出清的均衡。\n\n**起点**\n- 依赖 [[arro2026flight/1|a|a]]：两国环境。\n\n**均衡定义**\n1. 候选对象：价格与配置路径。\n2. 必须满足的条件：家庭最优且市场出清。\n\n**作用域**\n全模型。"
        }),
    );
    let crit_addr = crit["address"].as_str().unwrap().to_string();

    // Chapter 1 on main line (<head>|a|1).
    let ch1 = mcp.call(
        "create_successor",
        json!({
            "at": setting_addr,
            "kind": "direct",
            "facet": "第 1 章：区制依赖",
            "title": "第 1 章：区制依赖决定外逃传导方向",
            "body": "**直觉**\n便利收益外逃会通过跨国资产需求改变本国通胀和产出缺口。\n\n**本章结果**\n见侧支。\n\n**承上启下**\n依赖 [[arro2026flight/1|a|a|b]]：在两国竞争均衡下求解。"
        }),
    );
    let ch1_addr = ch1["address"].as_str().unwrap().to_string();

    // P2b-1 R6 gate check: writing forbidden term `流动性溢价` (and unlinked local proof FM3) in a
    // Level 2 formal part MUST be flagged by zt's own gate reading `<head>|a|a`'s **统一用语**!
    let r1 = mcp.call(
        "create_successor",
        json!({
            "at": ch1_addr,
            "kind": "side",
            "facet": "结果：命题 1 区制依赖",
            "title": "结果：命题 1 便利收益传导",
            "body": "本命题说明流动性溢价上升的影响。\n\n**陈述**\n- 出处与力度：Proposition 1，力度：characterization。\n- 条件与作用域：依赖 [[arro2026flight/1|a|a|b]]。\n$$\\pi = \\nu x$$\n\n**为什么**\n资产替代。\n\n**依据**\n- 局部证明：见下。\n\n来源：[[arro2026flight]] §2"
        }),
    );
    let r6_warnings = r1["warnings"].as_array().expect("warnings array");
    assert!(
        r6_warnings.iter().any(|w| w["rule"] == json!("R6") && w["message"].as_str().unwrap_or("").contains("流动性溢价")),
        "zt gate must flag R6 term from <head>|a|a: {r1}"
    );
    assert!(
        r6_warnings.iter().any(|w| w["rule"] == json!("FM3")),
        "zt gate must flag FM3 unlinked local justification on formal Result: {r1}"
    );
    let r1_addr = r1["address"].as_str().unwrap().to_string();

    let d1 = mcp.call(
        "create_successor",
        json!({
            "at": r1_addr,
            "kind": "side",
            "facet": "推导：便利收益楔子",
            "title": "推导：便利收益楔子",
            "body": "**起点**\n- 依赖 [[arro2026flight/1|a|a|b]]：由均衡欧拉方程出发。\n\n**步骤**\n1. 线性化债券欧拉方程。\n\n**结果**\n$$x = \\nu$$"
        }),
    );
    let d1_addr = d1["address"].as_str().unwrap().to_string();

    let w2 = mcp.call(
        "create_successor",
        json!({
            "at": d1_addr,
            "kind": "direct",
            "facet": "论证：泰勒原理排除爆炸解",
            "title": "论证：泰勒原理唯一性",
            "body": format!("目标：排除爆炸路径。\n\n**依据**\n- 依赖 [[{d1_addr}]]：由第一步系统出发。\n\n**论证**\n泰勒系数大于一，唯一有界解由稳定特征值决定。\n\n**结论**\n唯一有界解存在。")
        }),
    );
    let w2_addr = w2["address"].as_str().unwrap().to_string();

    let d3 = mcp.call(
        "create_successor",
        json!({
            "at": w2_addr,
            "kind": "direct",
            "facet": "推导：国内块缩放",
            "title": "推导：求解通胀响应",
            "body": format!("**起点**\n- 依赖 [[{w2_addr}]]：在唯一有界解下求解。\n\n**步骤**\n1. 代入国内菲利普斯曲线。\n\n**结果**\n$$\\pi = \\nu x$$")
        }),
    );
    let d3_addr = d3["address"].as_str().unwrap().to_string();

    // Add 边界 and 例示 under R1 (must NOT inflate proof.steps!).
    mcp.call(
        "create_successor",
        json!({
            "at": r1_addr,
            "kind": "side",
            "facet": "边界：零下界失效",
            "title": "边界：触及零下界时区制反转",
            "body": format!("依赖 [[{r1_addr}]]：当名义利率触及零下界时泰勒原理不再成立。")
        }),
    );
    mcp.call(
        "create_successor",
        json!({
            "at": r1_addr,
            "kind": "side",
            "facet": "例示：对称稳态特例",
            "title": "例示：对称国别特例",
            "body": format!("依赖 [[{r1_addr}]]：当两国规模相等时简化为半幅传导。")
        }),
    );

    // Update R1's **依据** to point to the actual route root D1 (and remove forbidden term).
    let r1_edited = mcp.call(
        "edit_card",
        json!({
            "at": r1_addr,
            "body": format!("稳态份额决定便利收益外逃的通胀方向。\n\n**陈述**\n- 出处与力度：Proposition 1，力度：characterization。\n- 条件与作用域：依赖 [[{crit_addr}]]。\n$$\\pi = \\nu x$$\n\n**为什么**\n跨国资产需求重配。\n\n**依据**\n- 局部证明：见 [[{d1_addr}]]，经 [[{w2_addr}]] 推至 [[{d3_addr}]]。\n\n来源：[[arro2026flight]] §2")
        }),
    );
    let empty_warn: Vec<Value> = Vec::new();
    let r1_fm_warns: Vec<&Value> = r1_edited["warnings"]
        .as_array()
        .unwrap_or(&empty_warn)
        .iter()
        .filter(|w| w["rule"].as_str().unwrap_or("").starts_with("FM"))
        .collect();
    assert!(r1_fm_warns.is_empty(), "clean edited formal Result must have zero FM warnings: {r1_edited}");

    // Inspect get_model output.
    let model = mcp.call("get_model", json!({ "at": head_addr }));
    assert!(model["build"]["formal_v2"].is_null(), "legacy formal_v2 flag must be removed: {model}");
    assert_eq!(
        model["build"]["claim_objects"],
        json!(["market-equilibrium", "dynamics-limit"]),
        "{model}"
    );
    assert_eq!(
        model["build"]["formal_shape"],
        json!([]),
        "clean formal model has zero formal_shape faults: {model}"
    );
    assert_eq!(model["build"]["setting_dependency"], json!([]));
    assert_eq!(model["build"]["result_unbound"], json!([]));
    assert_eq!(model["build"]["proof_unbound"], json!([]));
    assert_eq!(model["build"]["symbol_uncovered"], json!([]));
    let order = model["build"]["order"].as_array().expect("build.order");
    assert!(
        order.iter().any(|o| o["address"] == json!(r1_addr) && o["role"] == json!("result")),
        "build.order must emit canonical role for Level 2 formal parts: {order:?}"
    );

    // Side-branch maximal Derivation chains must enumerate [D1] and [D3] (broken at W2 论证).
    let chains = model["build"]["chains"].as_array().expect("chains");
    let chain_addrs: Vec<Vec<&str>> = chains
        .iter()
        .map(|c| {
            c["parts"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|v| v.as_str())
                .collect()
        })
        .collect();
    assert!(
        chain_addrs.contains(&vec![d1_addr.as_str()])
            && chain_addrs.contains(&vec![d3_addr.as_str()]),
        "mixed Side branch must split into [D1] and [D3]: {chain_addrs:?}"
    );

    // Check R1 child in chapter block: proof.steps == 3 (D1, W2, D3; excluding 边界 & 例示) and badge == result.
    let ch1_block = model["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["head"] == json!(ch1_addr))
        .expect("ch1 block");
    let r1_child = ch1_block["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["address"] == json!(r1_addr))
        .expect("r1 child");
    assert_eq!(
        r1_child["proof"]["steps"],
        json!(3),
        "proof.steps must count only 推导/论证 (3), excluding 边界 and 例示: {r1_child}"
    );
    assert_eq!(r1_child["badge"]["kind"], json!("result"), "{r1_child}");

    // 2. Test 1-hop `沿用 [[...]]` borrowed notation R6 gate on a second Level 2 formal model!
    let head2 = mcp.call(
        "create_successor",
        json!({
            "at": sec1,
            "kind": "side",
            "type": "model",
            "level": "formal",
            "title": "变体模型：借用符号表",
            "body": "问题：零下界下如何变化。\n\n**目标对象与标签**\n- 对象：market-equilibrium\n\n**原语与判据**\n沿用基准环境。\n\n**主要结果**\n见下。"
        }),
    );
    let head2_addr = head2["address"].as_str().unwrap().to_string();
    let set2 = mcp.call(
        "create_successor",
        json!({
            "at": head2_addr,
            "kind": "side",
            "facet": "设定",
            "title": "设定：零下界变体",
            "body": "引入名义利率零下界。\n\n**起点**\n- 原始设定：变体。\n\n**设定**\n名义利率非负。\n\n**直觉**\n- 是什么：下界约束。\n- 为什么需要：刻画流动性陷阱。\n- 怎样进入模型：限制货币政策。"
        }),
    );
    let set2_addr = set2["address"].as_str().unwrap().to_string();
    mcp.call(
        "create_successor",
        json!({
            "at": set2_addr,
            "kind": "side",
            "facet": "符号表与读法",
            "title": "符号表与读法",
            "body": format!("**怎样读卡片上的记号**\n沿用 [[{notation_addr}]] 的全部符号与用语。")
        }),
    );
    let borrow_r6 = mcp.call(
        "create_successor",
        json!({
            "at": set2_addr,
            "kind": "direct",
            "facet": "第 1 章：零下界",
            "title": "第 1 章：零下界效应",
            "body": "**直觉**\n此时流动性溢价直接压低通胀。\n\n**本章结果**\n无。\n\n**承上启下**\n依赖 [[arro2026flight/1|b|a]]。"
        }),
    );
    let borrow_warnings = borrow_r6["warnings"].as_array().expect("borrow warnings");
    assert!(
        borrow_warnings.iter().any(|w| w["rule"] == json!("R6") && w["message"].as_str().unwrap_or("").contains("流动性溢价")),
        "1-hop 沿用 borrowed notation must enforce R6 in zt gate: {borrow_r6}"
    );
    // 2b. An Assumption of Model 2 may rest on an IMPORTED Result from Model 1 without triggering setting_dependency!
    mcp.call(
        "edit_card",
        json!({
            "at": head2_addr,
            "body": format!("问题：零下界下如何变化。\n\n**目标对象与标签**\n- 对象：market-equilibrium\n\n**原语与判据**\n沿用基准环境。\n\n**引入**\n- 单卡 [[{r1_addr}]]：引入基准命题 1 的传导结论。\n\n**主要结果**\n见下。")
        }),
    );
    mcp.call(
        "create_successor",
        json!({
            "at": set2_addr,
            "kind": "side",
            "facet": "假设：区间条件",
            "title": "假设：借用基准结果界定参数区间",
            "body": format!("**起点**\n- 依赖 [[{r1_addr}]]：使用基准命题 1 的临界条件。\n\n**假设**\n参数位于该临界值以内。\n\n**直觉**\n- 是什么：区间限定。\n- 为什么需要：保证共存。\n- 怎样进入模型：约束参数域。")
        }),
    );
    let head2_model = mcp.call("get_model", json!({ "at": head2_addr }));
    assert_eq!(
        head2_model["build"]["setting_dependency"],
        json!([]),
        "setting/assumption resting on an IMPORTED Result from another model must not trigger setting_dependency: {head2_model}"
    );

    // 3. Unconditional L2 formal enforcement: a Level 2 formal model without notation card /
    // canonical Result parts MUST report Layer 1 formal_shape faults (no legacy bypass).
    let bare = mcp.call(
        "create_successor",
        json!({
            "at": sec1,
            "kind": "side",
            "type": "model",
            "level": "formal",
            "title": "缺项形式模型",
            "body": "问题：缺少符号表与结果卡的形式模型。"
        }),
    );
    let bare_addr = bare["address"].as_str().unwrap().to_string();
    let bare_s = mcp.call(
        "create_successor",
        json!({
            "at": bare_addr,
            "kind": "side",
            "facet": "设定",
            "title": "设定：缺项模型",
            "body": "缺少符号表分卡。\n\n**直觉**\n- 是什么：示例。\n- 为什么需要：验证。\n- 怎样进入模型：直接使用。"
        }),
    );
    let bare_s_addr = bare_s["address"].as_str().unwrap().to_string();
    mcp.call(
        "create_successor",
        json!({
            "at": bare_s_addr,
            "kind": "direct",
            "facet": "命题 1",
            "title": "命题 1：非规范分面",
            "body": format!("依赖 [[{bare_s_addr}]]：非规范结果。")
        }),
    );
    let bare_model = mcp.call("get_model", json!({ "at": bare_addr }));
    let bare_shape = bare_model["build"]["formal_shape"].as_array().expect("formal_shape");
    assert!(
        bare_shape.iter().any(|f| f["lacks"] == json!("符号表与读法"))
            && bare_shape.iter().any(|f| f["lacks"] == json!("结果")),
        "bare L2 formal model must report Layer 1 formal_shape faults unconditionally: {bare_model}"
    );
    // Editing the built bare head without formal sections must trigger FM1.
    let bare_edit = mcp.call(
        "edit_card",
        json!({
            "at": bare_addr,
            "body": "问题：仍然缺少正式结构节。"
        }),
    );
    let bare_fm: Vec<&Value> = bare_edit["warnings"]
        .as_array()
        .unwrap_or(&empty_warn)
        .iter()
        .filter(|w| w["rule"] == json!("FM1"))
        .collect();
    assert!(!bare_fm.is_empty(), "built L2 formal head must trigger FM1 unconditionally: {bare_edit}");

    // 4. Mathematical unit isolation: a formal Model Card with `form: "theorem"` receives
    // ZERO FM1–FM6 warnings and ZERO Level 2 formal Layer 1 faults.
    let math_unit = mcp.call(
        "create_successor",
        json!({
            "at": sec1,
            "kind": "side",
            "type": "model",
            "level": "formal",
            "form": "theorem",
            "title": "定理 1：连续函数有界",
            "body": "**陈述**\n闭区间上的连续函数有界。\n\n**思路**\n由紧性得证。"
        }),
    );
    let math_addr = math_unit["address"].as_str().unwrap().to_string();
    let math_part = mcp.call(
        "create_successor",
        json!({
            "at": math_addr,
            "kind": "side",
            "facet": "前提",
            "title": "前提：闭区间连续",
            "body": "函数在闭区间上连续。"
        }),
    );
    let math_fm: Vec<&Value> = math_part["warnings"]
        .as_array()
        .unwrap_or(&empty_warn)
        .iter()
        .filter(|w| w["rule"].as_str().unwrap_or("").starts_with("FM"))
        .collect();
    assert!(math_fm.is_empty(), "math unit part must receive zero FM warnings: {math_part}");
    let math_model = mcp.call("get_model", json!({ "at": math_addr }));
    assert_eq!(math_model["build"]["formal_shape"], json!([]), "{math_model}");
    assert_eq!(math_model["build"]["result_unbound"], json!([]), "{math_model}");
    assert_eq!(math_model["build"]["symbol_uncovered"], json!([]), "{math_model}");
}
