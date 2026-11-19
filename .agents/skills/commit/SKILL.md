---
name: commit
description: 변경을 커밋할 때 사용한다. 커밋할 파일에서 special-comments.toml이 허용하지 않은 주석을 remove-comments 스킬로 지운 뒤 커밋한다.
---

# 커밋

1. 커밋할 변경을 정한다. 포함할 변경에 근거가 없으면 AGENTS.md의 임의 판단 금지 규칙대로 질문한다.
2. 커밋할 파일을 stage한다.
3. `remove-comments` 스킬을 `--staged` 대상으로 실행한다.
4. 주석이 지워진 파일을 다시 stage한다. `special-comments.toml`에 패턴을 추가했다면 이 파일도 stage한다.
5. 도구로 `check --staged`를 다시 실행한다. 통과하지 않거나 건너뛴 파일이 있으면 커밋하지 않고, 파일과 이유를 보고한 뒤 어떻게 할지 질문한다.
6. 커밋한다.
7. 커밋 해시, 지운 주석과 docstring의 수, `special-comments.toml`에 추가한 패턴을 보고한다.
