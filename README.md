# commentless-agent-template

Codex와 Claude Code에서 쓰는 에이전트 지침과 skill을 담은 템플릿입니다. 지침은 [AGENTS.md](AGENTS.md)에 있습니다.

## 철학

주석은 코드가 바뀌어도 함께 고쳐지지 않아 낡고, 낡은 주석은 사람과 LLM의 판단을 흐립니다. 이 템플릿은 주석 대신, 바뀌면 반드시 함께 고쳐질 수밖에 없는 코드 자체에 의도를 담습니다. 이름은 길어지더라도 동작과 의미를 정확히 드러내고, 타입은 코드에 직접 씁니다.

또 사람의 요청이 모호할 때 LLM이 임의로 판단하지 않도록, 근거 없는 결정은 내리지 않고 질문하게 합니다.

## 지원 언어

C, C++, Python, Rust만 지원합니다.

## 허용된 주석

꼭 필요한 주석은 [`special-comments.toml`](special-comments.toml)에 패턴으로 등록된 것만 쓸 수 있습니다. 검사와 제거는 Rust로 만든 [`special-comments`](.agents/tools/special-comments) 도구가 하며, 빌드하려면 Rust와 C 컴파일러가 필요합니다.

## Skills

| Skill | 설명 |
| --- | --- |
| [`commit`](.agents/skills/commit/SKILL.md) | 커밋할 파일에서 허용되지 않은 주석을 지운 뒤 커밋합니다. |
| [`remove-comments`](.agents/skills/remove-comments/SKILL.md) | 허용되지 않은 주석과 Python docstring을 지웁니다. `commit`이 커밋할 파일에 대해 호출하며, 저장소 전체 정리에도 씁니다. 저장소 전체 정리는 필수가 아닌 권장 작업입니다. |
