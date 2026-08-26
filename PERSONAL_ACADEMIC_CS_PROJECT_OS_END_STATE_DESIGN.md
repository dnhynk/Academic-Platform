# 서울대학교 컴퓨터공학부 Personal Academic · CS · Project OS

## 이상적 최종 시스템 설계

| 항목 | 값 |
|---|---|
| 문서 성격 | 단일 사용자를 위한 end-state 제품·도메인·시스템 설계 |
| 조사 기준일 | 2026-08-26 (KST) |
| 사용자 | 서울대학교 컴퓨터공학부 학부생이자 풀스택 개발자 |
| 범위 | Academic · Lecture · CS Knowledge · Project · Career 전체 수명주기 |
| 범위 밖 | MVP, 개발 순서·기간, 시장성, 수익화, 다중 사용자 SaaS 운영 |
| 개인화 상태 | 입학년도·졸업기준년도·다전공 여부·성적표가 없어 `NOT_PERSONALIZED` |

> 이 문서에서 “확인됨”은 조사 기준일에 접근 가능한 공식 자료에서 확인했다는 뜻이지, 미래에도 변하지 않는다는 뜻이 아니다. 모든 학사 판정은 실제 사용자에게 적용되는 입학년도, 졸업기준, 경과조치와 최신 공식 원문을 다시 결합해야 한다.

---

## 1. Executive Definition

### 한 문장 정의

**서울대학교 컴퓨터공학부에서의 학사 이력, 원문 보존형 강의 기록, CS 개념과 실제 역량, 질문, 개인 코드베이스, 프로젝트 목표와 진로를 변경 불가능한 증거 원장과 버전된 개인 지식 그래프 위에서 연결하는 local-first Personal Academic · CS · Project OS**다.

### 한 문단 정의

이 OS는 사용자가 공부한 내용을 대신 설명하거나 결정을 대신 내리는 AI tutor가 아니다. 흩어진 공식 학사 사실, 수업에서 실제로 들은 말, 과제에서 수행한 일, 코드에서 직접 사용한 구조, 해결한 질문과 사용자의 확인을 각각 독립된 증거로 보존하고, 그 증거로부터 “나는 무엇을 접했고, 무엇을 할 수 있으며, 지금도 바로 꺼내 쓸 수 있는가”를 재구성하는 개인용 control plane이다. 그래프는 사실의 저장소 자체가 아니라 증거를 탐색 가능한 좌표계로 투영한 결과다. 학점·졸업 판정은 버전된 규칙으로 계산하고, 전사·개념 추출·repository 의미 분석은 불확실한 후보로 제시하며, 이해·질문 해결·수강·진로 선택의 마지막 결정은 사용자에게 남긴다.

### 시스템의 중심 불변조건

1. **원본보다 강한 파생물은 없다.** Audio, raw transcript, 공식 문서 snapshot, repository snapshot이 파생 PDF·그래프·점수보다 우선한다.
2. **Claim은 Evidence 없이 확정되지 않는다.** “안다”, “사용한다”, “필수다”, “개설된다”는 각각 근거·시점·적용 범위를 가진다.
3. **공식 사실, 관찰, 추론, 예측, 사용자 판단을 합치지 않는다.** 화면에서도 서로 다른 badge와 언어로 표시한다.
4. **수강 완료는 이해 완료가 아니다.** Course, Lecture, Assessment, Grade, Knowledge State, Competency는 독립 객체다.
5. **Mastery와 Freshness를 합치지 않는다.** 오래되었다는 이유로 실력을 자동 강등하지 않는다.
6. **Spec은 의도이고 Code/Runtime은 구현 증거다.** 둘의 충돌은 한쪽을 삭제하지 않고 drift로 보존한다.
7. **민감 데이터의 외부 반출은 명시적 권한이다.** provider 선택은 편의 설정이 아니라 데이터별 permission decision이다.
8. **녹음 권한이 불명확하면 녹음하지 않는다.** Record는 동의·정책 조건이 확인된 강좌에서만 활성화된다.
9. **AI는 상태를 제안할 수 있으나 사용자 결정을 덮어쓸 수 없다.** 충돌은 새 evidence로 표시한다.
10. **모든 projection은 재생성 가능해야 한다.** 그래프, 검색 인덱스, PDF, readiness view가 손상되어도 원장과 원본으로 복구할 수 있어야 한다.

```text
Official sources ─┐
Lecture sources ──┤
Project sources ──┼──> Local Personal Evidence Vault
User decisions ───┘       ├─ immutable artifacts
                           ├─ append-only events
                           ├─ atomic claims + evidence links
                           └─ canonical entity registry
                                      │
                 ┌────────────────────┼────────────────────┐
                 ▼                    ▼                    ▼
        deterministic views     graph projections    AI proposals
        GPA / requirements      CS / project / time  concepts / gaps
                 └────────────────────┬────────────────────┘
                                      ▼
                            explainable user decisions
```

### 핵심 구조 결정

- 단일 source of truth는 “그래프 DB”가 아니라 **로컬 Evidence Vault의 원본 artifact + append-only event/claim ledger**다.
- University, Knowledge, Project, Personal Graph는 **논리적으로 분리되고 공통 식별자와 typed edge로 결합**된다. 보안상 물리 저장·암호화 키·동기화 정책은 분리할 수 있다.
- 저장 기술은 교체 가능하다. Transactional store는 정합성, graph projection은 경로 탐색, full-text/vector projection은 검색을 담당하며 어느 projection도 원장이 아니다.
- 시스템은 “현재 답”과 함께 `왜`, `어느 시점 기준`, `무엇이 불확실한가`, `어떻게 고칠 수 있는가`를 항상 제공한다.

---

## 2. Product Philosophy

### 존재 이유

대학 생활과 개발 경력에는 학습 자체보다 그 전에 생기는 마찰이 많다. 강의의 누락 없는 기록, 교과과정 변경 추적, 졸업요건 계산, 개념 이름의 정규화, 질문과 자료의 연결, code와 spec 비교, 지금 필요한 선수지식 탐색이 그것이다. OS는 이 반복적 기록·탐색·검증·연결 비용을 줄여 사용자가 읽고, 생각하고, 구현하고, 결정하는 시간과 주의력을 되돌려준다.

### 설계 판단 질문

모든 기능은 다음 gate를 통과해야 한다.

> 이 기능이 사용자의 상황 인식과 선택 능력을 높이는가, 아니면 이해한 듯한 결과를 만들어 사고를 외주화하게 하는가?

후자라면 기본 동작을 바꾼다. 예컨대 AI 답변을 바로 보여주는 대신 질문의 선수개념, 관련 강의 구간, 원문 자료, project evidence를 먼저 보여준다. 요약이 유용하더라도 원문을 대체하지 않고 별도 `NavigationAid`로만 둔다.

### 제품 원칙

- **Agency before automation**: 추천은 선택지가 되고 결정은 사용자가 한다.
- **Evidence before score**: 숫자보다 근거 목록과 반례를 먼저 보여준다.
- **Lossless before concise**: 강의 문서는 보존본과 탐색 보조본을 분리한다.
- **Context before content**: “무엇을 공부할까”보다 “왜 지금 이것이 필요한가”를 연결한다.
- **Uncertainty is UI**: 불확실성을 내부 로그가 아니라 화면의 1급 정보로 만든다.
- **Local ownership**: export 가능한 개방 형식과 삭제 가능한 원본을 사용자가 소유한다.
- **Longitudinal truth**: 최신 상태만 남기지 않고 상태가 바뀐 증거의 역사를 보존한다.
- **Restraint**: streak, rank, 과도한 알림, 무근거 종합점수로 사용자를 조종하지 않는다.

---

## 3. User Model

### 사용자 상태

사용자는 한 사람뿐이지만 한 가지 역할만 갖지 않는다.

| 역할 | 반복적으로 마주치는 객체 | 필요한 도움 |
|---|---|---|
| SNU CSE 학부생 | 학번, 교육과정, 강좌, 수강, 성적, 졸업요건 | 규정 적용과 누락 검증 |
| 학습자 | 강의, 원문, 개념, 선수관계, 질문, 복습 | 이해를 막는 최소 gap 발견 |
| 풀스택 개발자 | spec, ADR, code, schema, infra, tests, incident | 구현과 이론의 근거 기반 연결 |
| 프로젝트 소유자 | 만들고 싶은 기능, 선택지, 기술부채, release | Build → Learn 경로 |
| 진로 탐색자 | role, competency, 관심과 비관심의 변화 | 증거 행렬과 대안 경로 |
| 장기 기록의 주체 | 수년간의 state, freshness, 질문, repo 변화 | time travel과 성장 서사 |

### 개인화에 필요한 최소 Profile

```yaml
StudentProfile:
  university: SNU
  college: CollegeOfEngineering
  department: CSE
  admissionYear: UNKNOWN
  selectedGraduationStandard: UNKNOWN
  degreeMode: UNKNOWN          # 단일전공 / 다전공 병행 등
  additionalMajors: []
  exchangeOrTransferCredits: []
  gradingContext: SNU_4_3
  interests: user_managed
  privacyPolicy: local_first_default_deny
```

알 수 없는 필드는 빈 문자열이 아니라 `UNKNOWN`으로 저장한다. 이 상태에서도 전체 OS는 작동하지만 졸업 판정은 `INDETERMINATE`이며, 임의의 학번을 선택해 결과를 확정하지 않는다.

### 사용자가 소유하는 결정

- 개념을 실제로 이해했는지와 어느 정도인지
- 질문이 해결되었는지, 또는 더 좋은 질문으로 바뀌었는지
- 강의·프로젝트·직무 중 현재 무엇을 중요한 목표로 삼는지
- 특정 Blind Spot을 탐색할지 의도적으로 제외할지
- 어떤 민감 자료를 어느 provider에 보낼지
- AI가 제안한 relation과 project classification을 승인·수정·거절할지

---

## 4. End-State Experience

### 하루

Home은 할 일 목록이 아니라 **오늘의 연결점**을 보여준다. 오전에는 오늘 강의와 녹음 권한 상태, 다음 강의에서 막힐 가능성이 큰 선수개념 1–3개가 보인다. 강의실에서는 사전 허가가 확인된 경우 한 번의 Record, Capture, Mark Moment만 사용한다. 수업 후에는 원본 audio와 모든 transcript segment가 보존된 lecture document가 생성되고, 낮은 STT confidence·수식·코드 구간만 검토 큐에 뜬다. AI가 concept와 question 후보를 제안하지만 사용자가 수업을 “이해 완료”했다고 자동 처리하지 않는다.

저녁에 repository를 열면 최신 snapshot과 이전 snapshot의 변화가 분석된다. 새 transaction 경로가 확인되면 `Transaction: OBSERVED`가 코드 줄·commit과 함께 나타난다. 동시 갱신 위험 때문에 `Isolation: REQUIRED` 후보가 생기고, 다음 주 Database lecture와 연결된다. 사용자는 concept page에서 강의 원문, assignment evidence, 관련 함수, 열린 질문을 한곳에서 본다.

### 한 학기

학기 전에는 실제 개설강좌만 `CONFIRMED`로 표시하고 과거 패턴은 `HISTORICALLY_LIKELY`로 분리한다. Planner의 안 A/B는 학점, 시간 충돌, 졸업규칙 proof, prerequisite readiness, project/career 관련성, workload의 범위와 출처를 나란히 보여준다. “수강하면 mastery +2” 같은 확정 예측은 하지 않고, 예상 가능한 `exposure opportunity`, `practice opportunity`, `assessment opportunity`를 시나리오로 표현한다.

학기 중에는 Course가 아니라 실제 CourseOffering 아래에 lecture, assignment, assessment, review가 쌓인다. 질문은 처음 생긴 맥락과 나중에 해결된 evidence를 유지한다. 기말 성적이 들어오면 GPA와 졸업 진행도는 즉시 재계산되지만 Knowledge State는 관련 evidence를 별도로 평가한다.

### 여러 학년

초기의 넓고 얕은 exposure는 강의·과제·프로젝트를 거치며 세부 concept과 competency evidence로 자란다. 2년 전 배운 Virtual Memory는 mastery가 유지된 채 freshness가 `STALE`로 보일 수 있고, 최근 성능 debugging으로 다시 높아진 과정이 time travel에 남는다. 관심 직무가 Backend에서 Systems로 바뀌어도 과거 경로를 지우지 않는다. 졸업 시점에는 성적표 이외에 “어떤 문제를 어떤 지식으로 실제 해결했는가”가 provenance와 함께 개인 CS 성장사로 남는다.

---

## 5. System Domains

| Domain | 핵심 Entity | 이 Domain이 답하는 질문 | 권위 원천 | 기본 민감도 |
|---|---|---|---|---|
| University | University, College, Department, Instructor, CurriculumVersion, DegreeRequirementSet | 어떤 공식 체계가 존재하는가 | 공식 원문 snapshot | Public |
| Academic | Course, CourseRevision, CourseOffering, Enrollment, Grade | 무엇을 언제 수강·이수했는가 | 공식 catalog + 사용자 성적표 | High |
| Lecture | Lecture, Audio, TranscriptSegment, Capture, Mark, LectureDocument | 실제 수업에서 무엇이 전달되었는가 | 원본 audio/image/material | Restricted |
| Knowledge | Concept, ConceptSense, Relation, KnowledgeState | 어떤 개념과 관계가 있으며 내 상태는 어떤가 | curated ontology + evidence-derived claim | High |
| Competency | Competency, PerformanceCriterion | 무엇을 할 수 있어야 하는가 | 관찰 가능한 수행 기준 | High |
| Evidence | Artifact, SourceRecord, EvidenceItem, Claim, Decision | 그 판단을 무엇이 뒷받침하는가 | immutable local ledger | Highest |
| Question | Question, Reframe, Resolution | 무엇이 막혔고 어떻게 변했는가 | 사용자 입력·연결 evidence | High |
| Project | Project, Repository, Snapshot, CodeComponent, Spec, ADR, Issue, Incident | 코드가 무엇을 사용·요구하는가 | immutable repo snapshot + analysis | Highest |
| Career | RoleProfile, CompetencyBundle, Goal | 목표 역할에 어떤 수행이 필요한가 | versioned bundle + user goals | High |
| Personal | StudentProfile, Preference, Override, Consent, InterestScope | 무엇이 이 사용자에게 적용되는가 | 사용자 결정 | Highest |

도메인은 소유권과 규칙을 분리하기 위한 bounded context다. 화면에서는 하나의 그래프로 보이지만, `Course`의 공식 제목을 project 분석기가 바꾸거나 AI concept extractor가 `Grade`를 쓰는 식의 경계 침범을 허용하지 않는다.

---

## 6. Core Domain Model

### 6.1 단일 source of truth: Personal Evidence Vault

SSOT는 다음 세 요소의 결합이다.

1. **Immutable Artifact Store**: audio, image, PDF, HTML snapshot, syllabus, transcript import, Git tree, spec, code file 등 원본 byte와 content hash.
2. **Append-only Event & Claim Ledger**: 언제 무엇을 관찰·추론·확인·거절했는지 기록하는 원장.
3. **Canonical Entity Registry**: 같은 실체를 가리키는 안정 ID, alias, merge/split history.

그래프, 관계형 view, 검색 index, vector embedding, PDF는 이 원장을 소비하는 재생성 가능한 projection이다. 따라서 graph database가 손상되거나 교체되어도 사실의 역사와 원본은 잃지 않는다.

```yaml
Artifact:
  id: art_...
  mediaType: audio/flac | text/html | application/pdf | git/tree
  sha256: ...
  byteLength: ...
  capturedAt: ...
  sourceLocator: ...           # URL, device, repo/commit 등
  confidentiality: PUBLIC | PERSONAL | RESTRICTED | SECRET
  encryptionDomain: ...
  retentionPolicy: ...

Claim:
  id: clm_...
  subjectId: ...
  predicate: ...
  object: entityId | scalar | structuredValue
  scope: ...                   # curriculum, offering, repo snapshot 등
  status: OFFICIAL_CONFIRMED | USER_CONFIRMED | CODE_OBSERVED |
          AI_INFERRED | PREDICTION | DISPUTED | SUPERSEDED
  confidence: null | 0.0..1.0
  validFrom: ...
  validTo: ...
  recordedAt: ...
  createdBy: user | deterministic_engine | model_run_id | importer
  evidenceLinks: [ev_...]
  supersedes: [clm_...]

EvidenceItem:
  id: ev_...
  artifactId: art_...
  locator: page | lineRange | transcriptSegment | repoPath+span | timestamp
  excerptHash: ...
  supportType: SUPPORTS | CONTRADICTS | CONTEXT_ONLY
  strength: DIRECT | CORROBORATING | WEAK
```

### 6.2 논리적 통합, 물리적 분리

```text
Logical coordinate system
┌─────────────────────────────────────────────────────────┐
│ shared stable IDs + typed claims + evidence links       │
└───────┬─────────────┬──────────────┬──────────────┬─────┘
        │             │              │              │
 University      Knowledge       Project        Personal/Lecture
 public/versioned curated        secret         highest sensitivity
        │             │              │              │
        └─────────────┴── unified graph projection ──┘
```

물리적으로는 공개 curriculum cache, 개인 학사 vault, lecture vault, repository vault를 별도 암호화 domain으로 둘 수 있다. 한 query가 여러 domain을 연결할 때 permission broker가 최소 projection만 메모리에서 조합한다. “물리적으로 하나의 거대한 graph store”는 단순하지만 private code와 공개 curriculum의 backup·sync·외부 처리 정책을 분리할 수 없어 채택하지 않는다.

### 6.3 주요 Aggregate와 경계

- `CurriculumAggregate`: CurriculumVersion, CourseRevision, RequirementSet, equivalency. 공식 snapshot 한 버전에 대해 원자적으로 publish한다.
- `CourseOfferingAggregate`: 한 학기 한 분반의 instructor, schedule, syllabus, lectures, assessments. Course catalog와 독립 lifecycle이다.
- `LectureAggregate`: 녹음 권한, audio chunks, segments, captures, marks, lossless document, coverage report.
- `KnowledgeAssertionAggregate`: concept에 대한 mastery/freshness claim과 evidence. evidence는 삭제하지 않고 새 assertion이 이전 것을 대체한다.
- `QuestionAggregate`: origin, revisions, relation, resolution decision. `RESOLVED`는 사용자 결정 event가 필요하다.
- `RepositorySnapshotAggregate`: immutable manifest와 그 snapshot에서만 유효한 code/spec/config claims.
- `DegreeAuditAggregate`: 특정 StudentProfile + RequirementSet + transcript snapshot에 대한 재현 가능한 proof tree.
- `PlanScenarioAggregate`: 사실 입력과 가정 입력을 동결한 what-if 결과. 실제 기록으로 자동 승격되지 않는다.

### 6.4 Entity identity, merge, split

Entity ID는 이름이 아니다. `MVCC`, `Multi-Version Concurrency Control`, `다중 버전 동시성 제어`는 하나의 `ConceptSense`에 붙는 alias일 수 있다. 반대로 `Cache`는 CPU cache, web cache, database buffer cache로 분리되어야 한다.

```text
raw mention "cache"
   └─ ALIAS_CANDIDATE_OF → con_cache_unspecified
           ├─ user/context confirms → con_cpu_cache
           └─ insufficient context  → remains unresolved mention
```

merge는 기존 ID를 지우지 않고 `MERGED_INTO` event와 redirect를 만든다. split은 과거 evidence를 자동 분배하지 않고 재분류 큐를 만든다. 그래야 ontology 개편이 과거 Knowledge State를 조용히 왜곡하지 않는다.

### 6.5 기술 중립적 논리 구성요소

| 구성요소 | 계약 | 특정 기술을 정답으로 두지 않는 이유 |
|---|---|---|
| Local Core | write transaction, encryption, policy, sync, audit | 개인 데이터 소유권의 중심 |
| Artifact Vault | content-addressed immutable blobs | DB row보다 큰 audio/git tree에 적합 |
| Canonical Store | entity, claim, rule, event의 강한 정합성 | graph만으로 회계·버전 규칙을 다루기 어려움 |
| Graph Projection | neighborhood, dependency, path, lens query | 원장이 아니라 탐색 최적화 view |
| Search Projection | full-text, symbol, semantic retrieval | embedding vendor와 독립 |
| Pipeline Runtime | STT, OCR, parsing, static analysis, AI jobs | provider 교체와 재현성 필요 |
| Policy Broker | data class × purpose × destination × retention 허가 | 보안을 UI 설정이 아닌 실행 gate로 만듦 |
| Client Surfaces | desktop/web/PWA/mobile capture/IDE | capture와 deep analysis의 장치 요구가 다름 |

관계형+재귀 query, native graph, embedded graph, vector store 중 어느 것을 택해도 된다. 선택 기준은 데이터 규모, bitemporal query, local backup, 암호화, offline 성능과 projection 재생성 비용이다. 벡터 검색은 유사성 후보를 찾을 뿐 identity나 prerequisite truth를 결정하지 않는다.

---

## 7. Knowledge Graph Semantics

### 7.1 Node 계층

```text
Entity
├─ Institution: University, College, Department
├─ Actor: Instructor
├─ Curriculum: CurriculumVersion, RequirementSet, Rule, Course, CourseRevision
├─ Activity: CourseOffering, Lecture, Assignment, Assessment, Enrollment
├─ Knowledge: Field, Concept, ConceptSense, Competency
├─ Evidence: Artifact, EvidenceItem, Claim, UserDecision
├─ Question: Question, Reframe, Resolution
├─ Engineering: Project, Repository, Snapshot, CodeComponent, Spec, ADR, Issue, Incident
└─ Goal: ProjectGoal, LearningGoal, RoleProfile, PlanScenario
```

Field와 Concept는 별개다. Database Systems는 cluster/field가 될 수 있고, Serializability는 설명·문제·코드에 직접 연결되는 concept다. Competency는 “개념을 안다”가 아니라 **관찰 가능한 상황에서 수행할 수 있다**는 문장으로 모델링한다.

### 7.2 Edge 방향과 엄밀한 의미

| Edge | 방향 | 의미와 사용 제한 |
|---|---|---|
| `REQUIRES` | advanced concept/competency/goal → prerequisite | 없으면 목표 수행이 신뢰성 있게 막히는 hard/near-hard dependency. 단순 선호는 금지 |
| `BUILDS_ON` | advanced → foundation | 이해를 깊게 하지만 반드시 선행해야 하는 것은 아닐 수 있음; `REQUIRES`와 구분 |
| `RELATED_TO` | canonical smaller-ID ↔ larger-ID | 비방향 연관. path engine의 prerequisite로 사용 금지 |
| `USED_IN` | concept → system/technique/context | 일반적으로 사용되는 곳. 개인이 적용했다는 증거는 아님 |
| `IMPLEMENTS` | code component/technique → abstraction/concept | 구체물이 추상 개념을 구현함 |
| `ABSTRACTS` | abstraction/API → lower mechanism/component | 세부를 감추는 계약; 단순 상위 분류가 아님 |
| `SPECIAL_CASE_OF` | specific → general | 논리적 subtype/specialization |
| `DESIGNED_TO_TEACH` | CourseRevision → concept/competency | 공식 설명·검토된 curriculum 의도 |
| `TAUGHT_IN` | concept → Lecture | 실제 특정 강의에서 설명됨. transcript/material evidence 필요 |
| `PRACTICED_IN` | concept/competency → Assignment/Practice | 사용·연습 기회가 있었음; 성공 의미 아님 |
| `ASSESSED_IN` | concept/competency → AssessmentItem | 실제 평가 대상. 성적과 mastery는 별도 |
| `APPLIED_IN` | concept/competency → ProjectSnapshot/CodeComponent | 사용자의 실전 적용 evidence가 있음 |
| `MENTIONED_IN` | concept/term → source segment | 언급만 됨; teaching/understanding으로 승격 금지 |
| `ENABLES_COMPETENCY` | concept → competency | 수행에 기여. 중요도와 필요/선택 구분 필요 |
| `RELEVANT_TO_ROLE` | competency/concept → RoleProfile | versioned role bundle에서의 관련성 |
| `OBSERVED_IN_PROJECT` | concept → ProjectSnapshot | code/config/test/runtime에서 강한 증거가 있음 |
| `REQUIRED_BY_PROJECT` | concept → ProjectSnapshot/Goal | 현재 안전한 이해·유지·요구 기능에 필요; failure chain 필요 |
| `BENEFICIAL_TO_PROJECT` | concept → ProjectSnapshot/Goal | 명시된 trigger가 생길 때 유익; 현재 필수 아님 |
| `RESOLVES_QUESTION` | EvidenceItem/Decision → Question | 해결 근거. 상태 변경은 사용자 decision과 별도 |
| `EVIDENCED_BY` | Claim/StateAssertion → EvidenceItem | claim이 의존하는 근거 |

역방향 탐색은 query view로 제공하며 반대 edge를 중복 저장하지 않는다. 예를 들어 `Concept TAUGHT_IN Lecture`의 inverse label은 UI에서 “이 강의가 가르친 개념”으로 표현한다.

### 7.3 Edge 자체도 Claim이다

```yaml
GraphAssertion:
  subject: concept_buffer_pool
  predicate: REQUIRES
  object: concept_disk_page
  prerequisiteStrength: HARD | STRONG | HELPFUL
  scope: database_learning_v3
  evidence:
    - lecture_04/segment_0184
    - textbook_ch09/page_311
  status: AI_INFERRED
  confidence: 0.91
  validFrom: 2026-09-15
  validTo: null
  recordedAt: 2026-09-15T20:11:04+09:00
  lastVerified: 2026-09-16
```

`confidence`는 참일 확률의 장식 숫자가 아니라 calibration된 assertion 신뢰도다. 공식 문서에 명시된 선수과목은 confidence 대신 `OFFICIAL_CONFIRMED`와 적용 scope를 쓰고, 예측에는 반드시 confidence와 근거 window를 쓴다.

### 7.4 Granularity 정책

- 독립적으로 설명·질문·evidence·prerequisite를 붙일 수 있을 때 별도 concept가 된다.
- 문서 한 번에만 등장한 세부 용어는 `Mention`으로 두고 자동 concept 승격하지 않는다.
- 지나치게 큰 “Database”는 Field/cluster이고, “B+ Tree node split”은 B+ Tree 아래의 concept 또는 operation이다.
- ontology curator가 제안한 merge/split은 영향받는 state, edge, question 수를 미리 보여준 뒤 사용자 승인한다.
- alias는 언어·약어·버전별 명칭을 담고, homonym은 `ConceptSense`로 분리한다.

---

## 8. SNU Academic Model

### 8.1 조사 기준일의 공식 사실 snapshot

다음은 **개인화된 졸업 판정이 아니라 2026-08-26에 확인한 공식 source snapshot**이다.

| 확인된 사실 | 시스템 표현 | 공식 근거 |
|---|---|---|
| 학사 졸업은 130학점 이상, 전체 및 전공 평점평균 각각 2.0 이상을 요구한다고 CSE 졸업규정 페이지가 안내 | `RequirementSet` 내 total credits와 GPA rules | [CSE 졸업 이수 규정](https://cse.snu.ac.kr/ko/academics/undergraduate/degree-requirements) |
| 2026학번 CSE 공통교육과정 표는 교양 49학점 이상과 영역별 조건을 제시 | `CurriculumVersion=SNU_CSE_2026_GENERAL` | [CSE 필수 교양 과목](https://cse.snu.ac.kr/ko/academics/undergraduate/general-studies-requirements) |
| 2026학번 전공 표준형태는 2026학번부터 적용하며 2025학번 이전은 종전 형태 적용 | version applicability + transition rule | [CSE 전공 이수 표준 형태](https://cse.snu.ac.kr/ko/academics/undergraduate/curriculum) |
| 공식 단일전공 표는 2025–2026학번에 전공 63학점, 전필 24학점과 전선 내규필수 5학점을 제시 | admission/selected-standard scoped rule bundle | [CSE 졸업 이수 규정과 첨부 표](https://cse.snu.ac.kr/ko/academics/undergraduate/degree-requirements) |
| 단일전공 첨부 표는 컴퓨터공학부 소속 학생이 입학년도 이후 기준 중 졸업기준을 선택할 수 있다고 명시 | `selectedGraduationStandard`를 admission year와 별도 저장 | [CSE 주전공(단일전공) 졸업규정 첨부](https://cse.snu.ac.kr/api/v1/file/1767846103580_%EC%A3%BC%EC%A0%84%EA%B3%B5%28%EB%8B%A8%EC%9D%BC%EC%A0%84%EA%B3%B5%29.pdf) |
| CSE 졸업 페이지는 2016학년도 이후 공대 신입생의 생명존중(자살예방) 교육 의무를 안내 | non-credit completion rule with admission-year scope | [CSE 졸업 이수 규정](https://cse.snu.ac.kr/ko/academics/undergraduate/degree-requirements) |
| 같은 페이지는 2008학년도 신입생부터 전공 1과목 이상을 포함한 외국어진행강좌 3과목 이상을 안내하며, 2012학번부터 대학영어를 제외 | count-with-constraints rule | [CSE 졸업 이수 규정](https://cse.snu.ac.kr/ko/academics/undergraduate/degree-requirements) |
| 공식 catalog는 Course code, 명칭, 학점, 학년, 교과구분을 제공 | `CourseRevision` source | [CSE 학부 교과과정](https://cse.snu.ac.kr/ko/academics/undergraduate/courses) |
| 2026 교과과정에는 기하모델링 폐지·고급컴퓨터그래픽스 대체, IT창업개론 폐지·대체 미지정 변경이 게시 | equivalency/sunset rule | [CSE 교과목 변경 내역](https://cse.snu.ac.kr/ko/academics/undergraduate/course-changes) |
| 2027학번부터 컴퓨터프로그래밍의 전필 제외가 공지됨 | future effective curriculum change | [2027학번 기준 컴퓨터프로그래밍 전필 제외 안내](https://cse.snu.ac.kr/community/notice/25220) |
| 2027학년도 1학기부터 `컴퓨터공학 학사논문연구`(3학점, S/U) 필수 이수 안내가 게시됨 | future effective graduation rule candidate | [CSE 졸업 이수 규정](https://cse.snu.ac.kr/ko/academics/undergraduate/degree-requirements) |
| 2026-2 수강신청 일정과 재수강 A0 상한 안내가 게시됨 | semester policy snapshot | [2026학년도 2학기 수강신청 안내](https://cse.snu.ac.kr/community/notice/25337) |
| SNU 등급은 A+ 4.3부터 F 0이며 S/U는 평점에서 제외 | versioned grading scheme | [서울대학교 성적등급 및 평점환산기준표](https://www.snu.ac.kr/academics/resources/certificate/grading) |

2026학번 교양 49학점 표의 세부 조건에는 글쓰기와 말하기 4학점, 외국어 6학점, 수학 16학점, 과학 선택필수 8학점, 컴퓨팅 3학점, 지성의 열쇠 3개 영역 9학점, 베리타스 3학점 등이 포함된다. 다만 면제, 동시수강 원칙, 입학 시 어학점수, 신설 교과목의 영역 인정 등 부가 규칙이 있으므로 단순 합계만으로 판정하지 않는다. 2026-08-07 공지 역시 교과목 영역 인정은 학생의 **입학년도 이수규정**을 기준으로 판단한다고 명시한다. [공통교육과정 영역 인정 기준 안내](https://cse.snu.ac.kr/community/notice/25379)

위 자료 사이의 날짜·표 제목·향후 효력 범위가 다르므로, “현재 CSE 규정”이라는 단일 row로 평탄화하면 안 된다. 특히 2027 학사논문연구의 정확한 적용 대상과 경과조치는 최종 졸업사정 전에 학부 공지·행정실 확인이 필요하다.

### 8.2 University Graph

```yaml
CurriculumVersion:
  id: cur_snu_cse_2026_major
  institutionPath: [SNU, CollegeOfEngineering, CSE]
  admissionYearRange: [2026, 2026]
  effectiveFrom: 2026-03-01
  effectiveTo: null
  status: OFFICIAL_CONFIRMED
  sourceSnapshot: art_cse_curriculum_2026
  supersedes: cur_snu_cse_2025_major

Course:
  id: course_snu_M1522_001800
  courseCode: M1522.001800
  canonicalIdentity: Database

CourseRevision:
  course: course_snu_M1522_001800
  titleKo: 데이터베이스
  credits: 3
  curriculumCategory: MAJOR_ELECTIVE
  officialPrerequisiteRules: [...]
  recommendedPrerequisiteClaims: [...]
  designedConceptCoverage: [...]
  designedCompetencyCoverage: [...]
  validFrom: ...
  sourceSnapshot: ...

CourseOffering:
  id: offering_...
  courseRevision: ...
  term: 2026_FALL
  section: ...
  instructors: [...]
  meetings: [...]
  capacity: ...
  gradingMode: ...
  syllabusArtifact: ...
  materialRefs: [...]
  lectureRefs: [...]
  assessmentRefs: [...]
  reviewRefs: [...]
  officialStatus: CONFIRMED
  observedAt: ...
```

Course는 시간에 걸친 교과목 정체성이고, CourseRevision은 명칭·학점·분류가 유효한 버전이며, CourseOffering은 실제 학기의 분반이다. 같은 code라도 revision이 바뀔 수 있고, 동일·대체 관계는 별도 effective-dated edge다.

### 8.3 개설 상태

| 상태 | 요건 | UI 문구 | Planner 취급 |
|---|---|---|---|
| `CONFIRMED` | 해당 학기 공식 수강편람/수강신청 시스템에 존재하고 최근 확인 | “공식 개설 확인 · 확인일” | 실제 시간표·정원 사용 |
| `HISTORICALLY_LIKELY` | 여러 과거 학기의 재현 가능한 패턴, 미래 공식 공지 없음 | “과거 패턴상 가능성” | placeholder만, 졸업계획 확정에 사용 금지 |
| `UNCERTAIN` | 표본 부족·불규칙·교수 변동 | “근거 부족” | 경고와 대체 경로 요구 |
| `CANCELLED/WITHDRAWN` | 공식 폐강·변경 공지 | “공식 취소” | 선택 불가, 과거 scenario 보존 |

공식 개설 확인은 [서울대학교 수강신청 시스템](https://sugang.snu.ac.kr/)의 최신 강좌 상세를 기준으로 하고, CSE 홈페이지·수강편람은 교차 출처로 사용한다. 2026-2 수강편람도 작성 기준일 이후 변경 가능하므로 수강신청 시스템의 최신 상태를 재확인하도록 공식 안내되어 있다.

역사 기반 예측은 최근 N개 학기의 단순 다수결이 아니다. 계절성(1/2학기), 교과목 신설·폐지·대체, 교수자 변화, 최근 공지, 미개설 gap, 불규칙 특강 여부를 feature로 사용하고, Course별 calibrated probability와 표본 window를 남긴다. 공식 향후 공지가 생기면 예측을 사실로 “승격”하지 않고 별도 official Claim을 활성화한다. 과거에 한 번도 관찰하지 못한 것은 `UNCERTAIN`이며 미개설 확정이 아니다. 예측 성능은 학기마다 Brier score/coverage와 abstention rate로 검증한다.

### 8.4 수집 대상과 source priority

1. 서울대학교 규정집·본부 공지: 학칙, 성적, 재수강, 전공 이수 공통 규정.
2. 공과대학 공식 자료: 공대 공통교과목, 공통교육, 별도 졸업 조건.
3. 컴퓨터공학부 공식 페이지·첨부: 학번별 전공/교양, 내규, 유사·대체 과목, 졸업논문.
4. 수강신청 시스템: 실제 CourseOffering, 시간표, 교수자, 정원, 강의계획서, 제한.
5. 교수자/LMS의 해당 Offering 자료: 실제 syllabus, 평가 방식, 녹음 정책.
6. 과거 이력에서 계산한 예측: 공식 사실과 분리된 `PREDICTION`.

source가 충돌하면 더 높은 번호/낮은 번호로 기계적 승자를 정하지 않는다. 규정의 법적 위계, 발령일, 적용일, 대상 scope, 경과조치를 비교하고 `ConflictCase`를 만든다. 졸업처럼 위험한 판정은 해결 전 `INDETERMINATE`로 둔다.

---

## 9. Course · Lecture · Assessment Model

### 경계

| 객체 | 정의 | 포함하지 않는 것 |
|---|---|---|
| `Course` | 대학이 지속적으로 식별하는 과목 | 특정 교수·학기·시간표·실제 설명 |
| `CourseRevision` | 일정 기간 유효한 제목·학점·공식 분류·설계된 coverage | 특정 분반의 현실 |
| `CourseOffering` | 학기·분반·교수자·시간·정원·syllabus의 실제 개설 | 매 수업시간의 실제 발화 |
| `Lecture` | 특정 날짜/시간에 실제 진행된 수업 세션 | 과목 전체 의도 |
| `LearningActivity` | `Assignment`, `ProjectAssignment`, lab, exercise | 평가 여부를 자동 포함하지 않음 |
| `Assessment` | `Quiz`, `Exam`, graded task의 평가 container | 한 문항의 concept coverage를 자동 가정하지 않음 |
| `AssessmentItem` | 실제 문항·rubric criterion | 전체 과목 mastery |

### 관계 예시

```text
CourseRevision(Database)
  DESIGNED_TO_TEACH ──> Transaction, Index, Recovery

Concept(B+ Tree)
  TAUGHT_IN ──────────> Lecture 05 / transcript segments 118–241
  PRACTICED_IN ───────> Assignment 2 / tasks 1–4
  ASSESSED_IN ────────> Midterm / question 3

Enrollment
  RECEIVED_GRADE ─────> A0

KnowledgeState(B+ Tree)
  EVIDENCED_BY ───────> lecture exposure, submitted solution,
                         user explanation, project experiment
```

Grade는 CourseOffering/Enrollment에 붙는다. Assessment item별 score가 있으면 더 정밀한 evidence 후보가 되지만, 좋은 총점으로 모든 concept을 `Understood`로 일괄 승격하지 않는다. 반대로 낮은 grade도 이후 project 적용 evidence를 무효화하지 않는다.

### lifecycle

```text
Course: ACTIVE ──> REVISED ──> RETIRED
Offering: ANNOUNCED ──> CONFIRMED ──> IN_PROGRESS ──> COMPLETED
                                └──> CANCELLED
Lecture: SCHEDULED ──> CAPTURE_AUTHORIZED ──> RECORDED ──> PROCESSED
                                                └──> PARTIAL/FAILED
Assessment: ANNOUNCED ──> SUBMITTED ──> GRADED ──> APPEAL/CLOSED
```

---

## 10. Personal Academic Record

### Enrollment/Attempt 모델

`TakenCourse` 하나로 재수강과 취소를 덮어쓰지 않고 매 시도를 보존한다.

```yaml
CourseAttempt:
  id: attempt_...
  studentProfile: me
  course: course_snu_...
  offering: offering_... | null
  term: 2026_FALL
  status: PLANNED | REGISTERED | IN_PROGRESS | COMPLETED |
          WITHDRAWN | CANCELLED | TRANSFERRED | RECOGNIZED
  creditsAttempted: 3
  creditsEarned: 3
  grade: A0 | S | U | W | I | null
  gradingScheme: snu_4_3_v...
  repeatOf: attempt_... | null
  repeatStatus: ORIGINAL | REPEAT | REPLACED | NOT_APPLICABLE
  requirementClassifications:
    - category: MAJOR_REQUIRED
      source: requirement_evaluation_...
  recognitionDecision: ...
  userNotes: ...
  sourceEvidence: transcript_page_...
```

`requirementCategory`는 시도에 사용자가 적어 넣은 영구 label이 아니다. 같은 과목이 적용 RequirementSet에 따라 전필·전선·일선으로 다르게 계산될 수 있으므로 rule engine이 생성한 versioned classification이다.

`PlannedCourse`는 CourseAttempt와도 분리한다. 아직 등록되지 않은 과목 후보는 `PlanScenarioChoice`가 Course 또는 예상 Offering을 참조하며, 실제 수강신청이 확인된 뒤에만 `CourseAttempt(status=REGISTERED)`를 만든다. 따라서 계획 삭제가 학사 이력을 지우거나, 계획만으로 졸업 actual progress가 올라가지 않는다.

### GPA 계산

기본 공식은 다음과 같지만 실제 포함/제외는 `GradingScheme`과 `RepeatPolicy` 버전이 결정한다.

```text
GPA = Σ(평점 대상 학점 × grade point) / Σ(평점 대상 학점)
```

SNU 공식 표는 A+ 4.3, A0 4.0, …, D- 0.7, F 0이며 S/U 교과목은 평점 계산에서 제외한다고 안내한다. 타 대학에서 2004학년도 이후 이수한 성적은 본교 평점평균에 산입되지 않는다는 유의사항도 있으므로, 인정학점과 GPA 포함 여부를 분리한다. [서울대학교 성적등급 및 평점환산기준표](https://www.snu.ac.kr/academics/resources/certificate/grading)

계산 view는 최소 다음을 제공한다.

- 누적 GPA와 계산에 포함된 attempt proof
- 학기별 GPA
- 전공 GPA: 적용 rule이 전공으로 분류한 시도만 사용
- 필요 시 다전공별 GPA
- 총 취득학점과 GPA denominator의 차이
- 재수강 전후 시도와 어느 성적이 인정되었는지
- S/U, W, I, F, 교환·편입·인정학점 처리 이유

2026-2 공식 수강안내는 학사과정 재수강 취득성적 상한을 2015학년도 1학기 이수 교과목부터 A0로 제한한다고 안내한다. 자격(C+ 이하), 옛 과목 경과조치, 동일·대체 매핑은 별도 versioned policy로 관리하고 최신 원문을 확인한다. [2026학년도 2학기 수강신청 안내](https://cse.snu.ac.kr/community/notice/25337)

### Academic Performance와 Knowledge 분리

Academic Dashboard에서 GPA chart와 Knowledge Map을 같은 카드의 한 score로 합치지 않는다. grade는 “그 Offering의 평가 체계에서 받은 결과”이고, Knowledge State는 concept-specific evidence synthesis다. 양쪽을 연결해 탐색할 수는 있지만 인과나 동일성을 주장하지 않는다.

---

## 11. Graduation Requirement Engine

### 11.1 RuleSet 선택

```yaml
DegreeRequirementSet:
  id: drs_snu_coe_cse_single_2026_v3
  institutionPath: [SNU, CollegeOfEngineering, CSE]
  admissionYear: 2026
  selectedGraduationStandardRange: [2026, 2026]
  majorMode: SINGLE_MAJOR
  effectiveFrom: 2026-03-01
  effectiveTo: 2027-02-28
  sourceArtifacts: [...]
  publicationStatus: OFFICIAL_CONFIRMED
  rules: [...]
  transitionRules: [...]
```

selector는 대학·단과대·학부·입학년도·사용자가 적법하게 선택한 졸업기준·주전공/복수/부/연합/연계·교환/편입·예외 승인을 함께 사용한다. 하나라도 필수 입력이 없거나 두 RuleSet이 경쟁하면 임의 선택하지 않고 `INDETERMINATE`와 필요한 확인 항목을 반환한다.

### 11.2 Typed rule DSL

```yaml
- id: total_credits
  type: CREDIT_MINIMUM
  category: ALL_RECOGNIZED
  threshold: 130

- id: cse_major_total
  type: CREDIT_MINIMUM
  category: CSE_MAJOR
  threshold: 63

- id: required_course_set
  type: ALL_OF
  operands:
    - COURSE_OR_EQUIVALENT: course_discrete_math
    - COURSE_OR_EQUIVALENT: course_data_structures

- id: seminar_choice
  type: AT_LEAST_N_OF
  n: 1
  operands: [course_cse_seminar, course_computing_overview]

- id: foreign_language_lectures
  type: COUNT_WITH_CONSTRAINTS
  minimum: 3
  constraints:
    - atLeastMajorCourses: 1
    - exclusionsByAdmissionYear: ...

- id: overall_gpa
  type: GPA_MINIMUM
  scope: ALL_GPA_ELIGIBLE
  threshold: 2.0
```

rule type에는 credit minimum, course set, area distribution, co-requisite, mutually exclusive, equivalency, maximum recognition, GPA, non-credit training, language-of-instruction, thesis/research, exception approval를 포함한다. 자유 텍스트를 LLM이 매번 해석해 졸업 여부를 판단하는 구조는 금지한다. LLM은 원문에서 rule 후보를 추출할 수 있으나 사람이 검토한 executable rule만 production audit에 사용한다.

### 11.3 설명 가능한 proof tree

```text
DegreeAudit: INDETERMINATE
├─ Total credits: 93 / 130                         PASS_PARTIAL
├─ CSE major total: 51 / 63                       NEEDS 12
├─ Major required set
│  ├─ Data Structures: satisfied by attempt_17    PASS
│  └─ Algorithms: planned only                    NOT_SATISFIED
├─ General education area: cannot evaluate
│  └─ admissionYear missing                       UNKNOWN
└─ Thesis research rule: applicability unresolved UNKNOWN
   └─ official notice effective 2027-1; scope confirmation needed
```

모든 leaf에는 적용 rule ID, source page/paragraph, 사용한 CourseAttempt, equivalency decision이 붙는다. 사용자는 숫자뿐 아니라 “왜 이 학점이 포함/제외되었는가”를 열 수 있다.

### 11.4 버전과 변경 감지

- 공식 원문을 retrieval time, effective time, hash와 함께 snapshot한다.
- 새 문서와 이전 문서를 구조·텍스트 diff하고 영향받는 rules를 표시한다.
- 변경은 기존 RuleSet을 수정하지 않고 새 버전을 publish한다.
- 과거 audit은 당시 입력과 rule hash로 재현한다.
- 동일·대체·폐지·경과조치는 독립 rule이며 양방향 동일성으로 단순화하지 않는다.
- 새 rule은 공식 예시와 synthetic transcript fixture로 회귀 검증한다.
- 고위험 결과(졸업 가능/불가)는 rule coverage 100%, unresolved conflict 0, source freshness 기준 충족 시에만 `DETERMINATE`가 된다.

현재 사용자의 입학년도가 없으므로 이 문서는 130학점 등 공개된 공통 사실을 예시로 사용할 뿐, 개인의 “남은 학점”을 산출하지 않는다.

---

## 12. Lecture Intelligence System

### 12.1 녹음 전 Permission Gate

강의 녹음은 기본 `UNKNOWN`이다. 정부 저작권 안내는 교수자 허락 없는 강의 녹음·녹화가 복제권을 침해하며, 사전에 허락받은 이용 방법과 조건 안에서 사용해야 한다고 명시한다. [대한민국 정책브리핑의 대학생 저작권 안내](https://www.korea.kr/news/policyNewsView.do?newsId=148928381) 따라서 “개인 학습이니 항상 가능”을 제품 가정으로 두지 않는다.

```yaml
CapturePermission:
  offering: offering_...
  status: UNKNOWN | PROHIBITED | PERMITTED | PERMITTED_WITH_CONDITIONS | EXPIRED
  grantedBy: instructor | institution | accessibility_accommodation
  evidenceArtifact: syllabus/email/announcement/user_attestation
  allowedMedia: [AUDIO, PHOTO_OF_BOARD]
  allowedProcessing: [LOCAL_STT, LOCAL_OCR]
  externalProcessingAllowed: false
  sharingAllowed: false
  retentionUntil: ...
  conditions: ...
  verifiedAt: ...
```

`UNKNOWN`이나 `PROHIBITED`이면 Record는 fail-closed다. 교수자 허락 외에도 syllabus/LMS의 과목별 정책, 학생 질문·발표가 녹음되는지, 화면 촬영 범위, 장애학생 지원 절차, 저작권·개인정보·학교 규정을 확인한다. 동의가 한 학기 전체인지 단일 강의인지 scope도 기록한다. 법률 판단이 필요한 예외는 시스템이 추정하지 않고 학교 담당부서 또는 전문가 확인 항목으로 남긴다.

`user_attestation`은 사용자가 교수자의 구두 허락을 언제 어떤 조건으로 들었는지 기록하는 증거 형식이지, 사용자가 스스로 허가를 만들어내는 override가 아니다. “개인 사용이므로 괜찮을 것” 같은 자기 판단은 permission을 `PERMITTED`로 전이시키지 못한다. 구두 허락의 범위가 모호하면 `PERMITTED_WITH_CONDITIONS` 또는 `UNKNOWN`을 유지하고 서면 확인을 요청하도록 돕는다.

### 12.2 저마찰 capture UX

```text
┌──────────────────────────────────────────┐
│ Database · Lecture 06      42:18         │
│ Permission: permitted / local only       │
│                                          │
│             [ Capture ]                  │
│             [ Mark Moment ]              │
│                                          │
│ Mark: 중요 / 이해 안 됨 / 질문 / 복습 / 강조 │
└──────────────────────────────────────────┘
```

- 화면은 잠금 상태·한 손 조작·무음 haptic을 지원한다.
- Capture는 원본 image, orientation, timestamp, audio clock offset을 저장한다.
- Mark Moment는 먼저 한 번의 표시만 저장하고, 세부 label은 수업 후 붙일 수 있다.
- 연결이 끊겨도 장치 로컬 chunk에 계속 기록한다.
- storage/battery/microphone failure를 즉시 비침습적으로 알린다.

### 12.3 Provider-neutral pipeline

```text
authorized audio chunks + captures + supplied materials
              │
              ▼
      TranscriptionProvider
      - local or explicitly permitted remote
      - word timestamps / confidence / diarization
              │
              ▼
      NormalizedTranscript vN
      - immutable raw provider output retained
      - corrected tokens are new versions
              │
              ├──> LosslessLectureDocument + PDF
              ├──> CoverageValidator
              └──> proposal jobs
                   concepts / relations / questions / gaps
```

Provider contract는 audio format, chunk boundary, language hints, vocabulary hints, word/segment timestamps, confidence semantics, diarization, math/code capability, data retention, training use, region, deletion receipt를 선언한다. provider 교체 시 raw provider response와 model/version을 보존하여 재전사 결과를 비교한다.

### 12.4 Normalized Transcript

```yaml
TranscriptSegment:
  id: raw_segment_0184
  lectureId: lecture_06
  startMs: 2529100
  endMs: 2543800
  speaker: instructor | student_unknown_2 | unresolved
  verbatimText: "..."
  tokens:
    - text: "serializability"
      confidence: 0.63
      startMs: ...
  sourceAudioChunks: [chunk_0042]
  correctionStatus: NEEDS_REVIEW
  versions: [...]
```

문장부호·문단·화자 label·수식 formatting은 별도 annotation layer다. 원문 token을 파괴적으로 바꾸지 않는다. 학생 이름 등 민감 발화는 display/redacted projection에서 숨길 수 있으나 원본 삭제 여부는 retention policy와 권리 요청에 따라 처리한다.

### 12.5 Lossless Lecture Document

PDF는 읽기 좋은 **보존형 rendering**이며 source of truth가 아니다. machine-readable `LectureDocument`가 section, paragraph, equation, code block, capture placement와 source segment mapping을 가진다.

```yaml
LectureParagraph:
  id: lecture_section_07_paragraph_03
  renderedText: ...
  sourceSegments:
    - segmentId: raw_segment_0184
      charRange: [0, 143]
      transform: PUNCTUATION_AND_FORMATTING
  nearbyCaptures: [capture_04218]
  annotations: [INSTRUCTOR_EMPHASIS, LOW_STT_CONFIDENCE]
```

허용되는 것은 순서 보존, 문장부호, section heading, timestamp, speaker, 수식/코드 formatting, 전문용어 표시, 반복·강조 annotation, capture 배치다. 반복 설명·사례·여담처럼 “덜 중요해 보이는” 발화를 삭제하지 않는다. 요약이 필요하면 별도 `StudyIndex`로 만들고 PDF의 대체물이 아님을 표시한다.

### 12.6 정보 손실 검증

CoverageValidator는 deterministic하다.

```text
segment coverage = mapped non-silence transcript segments / all eligible segments
token coverage   = mapped normalized tokens / all normalized tokens
ordering check   = source timestamp order is monotonic unless explicitly cross-referenced
capture check    = every authorized capture is placed or explicitly excluded with reason
gap check        = audio chunk timeline has no unexplained hole above threshold
```

- 모든 segment는 `MAPPED`, `EXCLUDED_NON_SPEECH`, `REDACTED_WITH_POLICY`, `UNTRANSCRIBED_FAILURE` 중 하나여야 한다.
- `UNMAPPED` 하나라도 있으면 PDF 상태는 `INCOMPLETE`; “완성” badge를 주지 않는다.
- text normalization 전후에는 token alignment와 diff report를 만든다.
- 수식·코드·낮은 confidence 구간은 원 audio, capture, slide와 함께 review queue에 둔다.
- 문서 render 후 page overflow, 잘린 code, 누락 image, 깨진 glyph를 검사한다.
- 사용자가 정정하면 provider 원문을 덮지 않고 corrected transcript version을 추가한다.

### 12.7 다음 강의 준비

syllabus, 다음 title/slide, 교재 chapter, LMS 자료, 과제, 공지, 직전 강의 말미에서 `ExpectedConceptClaim`을 추출한다. 이것을 Knowledge State와 prerequisite graph에 비교해 **이해를 막을 가능성이 큰 최소 기초**만 제안한다.

```text
Tomorrow: Database / Buffer Management
Expected: Disk Page, Buffer Pool, Replacement

blocking evidence
Disk Page       mastery: Exposed, freshness: Low
Memory Hierarchy mastery: Understood, freshness: Moderate

minimum preparation candidate
Disk Page (25–40 min) → Buffer Pool vocabulary (10–15 min)

not included
full lecture preview, advanced replacement-policy survey
```

예상 concept, prerequisite edge, 사용자 state가 모두 불확실할 수 있으므로 각각의 근거와 confidence를 분리한다.

---

## 13. Knowledge State & Freshness

### 13.1 Mastery는 학습 깊이, Confidence는 추정의 확실성

사용자에게는 이해하기 쉬운 6단계를 제공하되 내부에는 수행 차원을 함께 보존한다.

| Level | 이름 | 관찰 가능한 의미 | 자동 부여 한계 |
|---:|---|---|---|
| 0 | `UNSEEN` | 접했다는 evidence가 없음 | evidence 없음이지 “모른다”는 시험 결과가 아님 |
| 1 | `EXPOSED` | 강의·문서에서 의미 있게 접함 | mention 하나만으로는 부족할 수 있음 |
| 2 | `UNDERSTOOD` | 자신의 말로 설명하고 구분·예측 가능 | 강의 수강만으로 자동 승격 금지 |
| 3 | `PRACTICED` | 문제·과제·실험에서 사용 | 제출만이 아니라 해당 concept 수행 evidence 필요 |
| 4 | `APPLIED` | 실제 project의 결정·구현·debugging에 사용 | dependency 설치만으로 금지 |
| 5 | `FLUENT` | 새 상황에 독립적으로 전이·설계·설명 | AI 단독 판정 금지, 반복된 강한 evidence와 사용자 확인 필요 |

단일 level이 정보를 압축하므로 내부에는 다음 facet을 둔다.

```yaml
KnowledgeStateAssertion:
  concept: concept_transaction
  asOf: 2027-03-21
  masteryLevel: APPLIED
  facets:
    recognize: STRONG
    explain: MODERATE
    solveStructuredProblem: STRONG
    implementOrOperate: STRONG
    transferToNovelSituation: LIMITED_EVIDENCE
  estimateConfidence: 0.78
  freshnessBand: VERY_HIGH
  freshnessConfidence: 0.92
  userConfirmed: false
  evidence: [lecture_ev, assignment_ev, project_ev, debug_ev]
  contradictingEvidence: []
```

`estimateConfidence`는 사용자의 실력 점수가 아니다. 현재 mastery projection을 뒷받침하는 evidence의 충분성·일관성에 대한 시스템 확신이다. “mastery 4, confidence 0.45”는 applied evidence 후보가 있지만 authorship이나 수행 결과가 불명확함을 뜻한다.

### 13.2 Evidence가 Mastery에 미치는 영향

| Evidence | 허용되는 기본 해석 | 자동 상한 |
|---|---|---:|
| transcript에서 meaningful teaching | 접함 | Exposed |
| 사용자 자신의 설명 + 자기 확인 | 설명 가능 | Understood |
| concept-specific 과제 풀이·실험 성공 | 구조화된 적용 | Practiced candidate |
| 직접 작성한 production/personal project code와 test | 현실 맥락 적용 | Applied candidate |
| incident debugging에서 원인 규명·수정·검증 | 진단과 적용 | Applied, transfer facet 강화 |
| 서로 다른 맥락에서 반복 독립 수행·설계 | 전이 가능성 | Fluent candidate |
| dependency/install/import만 존재 | 기술 접점 | mastery 승격 없음 |
| 과목 grade | 광범위한 performance signal | concept별 직접 승격 없음 |

“자동 상한”은 안전한 기본값이다. user confirmation이나 더 직접적인 evidence가 있으면 올라갈 수 있다. 반대로 제출한 과제가 타인의 풀이를 복사한 것이라면 evidence를 철회할 수 있다. 철회 event도 역사에 남고 projection만 다시 계산한다.

### 13.3 Freshness는 즉시 인출 가능성의 별도 축

Freshness는 `VERY_HIGH`, `HIGH`, `MODERATE`, `LOW`, `STALE`, `UNKNOWN` band로 표시한다. 계산 입력은 다음과 같다.

- 마지막 strong evidence의 시점과 종류
- 최근 일정 window의 반복 횟수와 간격
- 노출·복습보다 실제 적용·debugging·설계에 더 긴 지속성
- 관련 concept의 최근 사용에서 오는 약한 spillover
- 사용자 직접 “지금도 바로 사용할 수 있음/복습 필요” 확인
- concept별 retention profile과 사용자별 경험적 보정
- 반대 evidence: 설명 실패, 기억 안 남음 표시, 재학습 필요 event

시간 decay는 freshness projection에만 적용한다. mastery를 자동 내리지 않는다.

```text
Mastery: Understood
Last strong evidence: 2025-07-12
Recent use: none
Freshness: STALE
Meaning: 과거 이해 evidence는 유지되지만 즉시 인출 가능성은 검증되지 않음
Action: 필요 시 15분 retrieval check 제안; “모름”으로 표시하지 않음
```

Freshness 함수는 concept별 half-life를 절대 진리로 두지 않는다. 초기값은 prior이고 실제 사용자의 회상 확인으로 calibration한다. 관련 concept 사용의 전파는 한 단계, 낮은 weight, 명시적 근거로 제한해 연쇄적으로 전체 분야가 신선해지는 오류를 막는다.

### 13.4 상태 갱신

```text
new evidence
   ↓
evidence classifier proposal
   ↓
deterministic eligibility checks
   ├─ exact concept linked?
   ├─ user authorship/participation known?
   ├─ outcome known?
   └─ source integrity valid?
   ↓
new KnowledgeStateAssertion (never in-place mutation)
   ↓
user accept / edit / leave unconfirmed / reject
```

사용자가 직접 확인한 state는 AI가 낮추거나 높이지 못한다. 이후 모순 evidence가 생기면 “사용자 확인과 새 evidence가 충돌”하는 review card를 만들고 양쪽을 보여준다.

---

## 14. Question Graph

### 14.1 Question은 메모가 아니라 시간성을 가진 지식 객체

```yaml
Question:
  id: q_...
  canonicalText: "왜 DB index에는 일반 BST보다 B+ Tree를 사용하는가?"
  createdAt: 2026-10-07T14:42:18+09:00
  origin:
    type: LECTURE
    entity: lecture_db_06
    locator: audio@42:18
  relatedConceptClaims:
    - B_PLUS_TREE
    - DISK_PAGE
    - RANDOM_IO
    - FAN_OUT
  status: OPEN
  importance: user_set | context_derived
  revisions: [...]
  resolutionDecision: null
```

origin은 Lecture, CourseMaterial, Assignment, PersonalStudy, Repository, CodeReview, ProjectSpec, ConceptDetail 중 하나며, repository origin은 snapshot/path/line까지 고정한다.

### 14.2 Lifecycle

```text
OPEN ───────────> PARTIALLY_RESOLVED ───────────> RESOLVED
 │                        │                           │
 ├──> REFRAMED ───────────┴──> new Question          ├──> REOPENED
 └──> OBSOLETE                                        └──> REFRAMED
```

- AI는 `resolution candidate`와 evidence를 제안할 수 있다.
- `RESOLVED`는 사용자의 명시적 decision 또는 사용자가 미리 정한 검증 행위 완료 후 사용자 승인으로만 전이한다.
- obsolete는 잘못된 가정·기술 변경 등으로 질문 자체가 더는 유효하지 않을 때 사용하며, “답하기 귀찮음”과 구분한다.
- reframe은 옛 질문을 수정해 지우지 않고 `REFRAMED_AS` edge로 새 질문에 연결한다.

### 14.3 답을 대신하지 않는 지원

질문 workspace의 기본 순서는 다음과 같다.

1. 질문이 생긴 원문 구간과 당시 화면/capture
2. 연결된 concept과 prerequisite
3. 이미 가진 evidence 중 관련성이 높은 것
4. 다시 등장하는 Lecture/Course/Project 위치
5. 교재·공식 문서·experiment 같은 가능한 resolution source
6. 원할 때만 AI explanation, 그리고 원문과 구분된 생성물

AI answer는 `GeneratedExplanation` artifact이며 resolution evidence가 될 수는 있어도 질문을 자동 닫지 않는다.

### 14.4 질문 변화로 보는 성장

질문을 난이도 점수로 줄 세우지 않는다. 시간축에서 다음 descriptor의 변화와 근거를 보여준다.

- 대상 범위: 용어 → 메커니즘 → trade-off → system boundary
- prerequisite depth: 기본 정의 → 상호작용 → failure mode
- 비교의 질: “무엇인가” → “왜 A 대신 B인가”
- 조건 명시: 무조건적 질문 → workload/failure assumptions가 있는 질문
- evidence 사용: 추상 질문 → code/trace/experiment에서 출발한 질문
- 재사용: 한 답이 다른 project나 concept에 전이된 횟수

예를 들어 Transaction 정의 질문이 MVCC write skew, 나중에는 분산 snapshot isolation 범위 질문으로 이어지는 chain을 보여주되 “질문 수준 +37%” 같은 허위 정밀도는 사용하지 않는다.

---

## 15. Gap & Prerequisite Engine

### 15.1 Gap의 정의

Gap은 낮은 Knowledge State 자체가 아니라 **활성 목표의 성공을 가로막는, 근거가 있는 prerequisite 부족**이다.

```yaml
GapCase:
  goal: understand_buffer_pool
  surfaceConcept: BUFFER_POOL
  rootCandidates:
    - concept: DISK_PAGE
      blockingPath: [BUFFER_POOL, DISK_PAGE]
      reason: "page가 I/O와 buffer frame의 교환 단위이기 때문"
      evidence: [lecture_segment, prerequisite_claim]
      confidence: 0.86
    - concept: STORAGE_HIERARCHY
      blockingPath: [BUFFER_POOL, DISK_PAGE, STORAGE_HIERARCHY]
      confidence: 0.61
  userStateSnapshot: ks_...
  minimumRemediationPaths: [...]
```

### 15.2 탐지 과정

1. 활성 목표를 concept/competency success criteria로 명시한다.
2. `REQUIRES`와 강한 `BUILDS_ON` subgraph를 확장한다.
3. 사용자 mastery, freshness, confidence와 contradicting evidence를 overlay한다.
4. 표면 concept에서 아래로 내려가며 최초의 강한 부족과 그 조상 영향도를 찾는다.
5. root 후보가 여러 개면 모두 유지하고 짧은 diagnostic activity를 제안한다.
6. hard gap, refresh gap, evidence gap, terminology mismatch를 구분한다.

| Gap 종류 | 뜻 | 예시 대응 |
|---|---|---|
| `MASTERY_GAP` | prerequisite 수행 evidence가 부족 | 기초 설명·문제·실험 |
| `FRESHNESS_GAP` | 과거 mastery는 있으나 즉시 사용 불확실 | 짧은 retrieval/refresher |
| `EVIDENCE_GAP` | 실제로 알 수 있으나 시스템에 근거가 없음 | 사용자 확인 또는 diagnostic |
| `ONTOLOGY_GAP` | synonym/granularity 오류로 잘못 분리됨 | merge/sense correction |
| `CONTEXT_GAP` | 목표나 구현 선택이 불명확해 prerequisite가 갈림 | 선택지와 조건 명확화 |

### 15.3 설명 계약

모든 Gap 제안은 `무엇`, `왜 막는가`, `근거`, `confidence`, `현재 상태`, `최소 보강`, `대체 경로`, `연결된 강의/프로젝트`를 포함한다. “데이터베이스를 더 공부하세요”는 너무 넓어 유효한 Gap 설명이 아니다.

---

## 16. Critical Path Engine

### 16.1 문제 정의

Critical Path는 concept 수가 가장 적은 shortest path가 아니다. 목표가 competency나 project capability일 수 있고, prerequisite가 AND/OR 구조이며, 이미 아는 것·낡은 것·현재 열리는 강의·즉각적인 project 가치가 다르다. 따라서 graph가 아니라 **typed prerequisite hypergraph 위의 constrained multi-objective planning**으로 다룬다.

```text
Goal: reliable real-time collaboration
  REQUIRES ALL [failure model, shared-state semantics]
  REQUIRES ONE OF
    ├─ [OT fundamentals, central server ordering]
    └─ [CRDT fundamentals, merge semantics]
```

### 16.2 비용 벡터

각 path는 단일 점수 대신 다음 벡터를 가진다.

```text
Cost(P) = <
  learning_effort,
  refresh_effort,
  prerequisite_risk,
  uncertainty,
  calendar_delay,
  context_switching,
  opportunity_cost
>

Benefit(P) = <
  goal_coverage,
  immediate_project_value,
  curriculum_value,
  reuse_across_goals,
  evidence_opportunity
>
```

사용자의 slider는 이 벡터를 정렬하는 preference일 뿐 진리를 바꾸지 않는다. engine은 먼저 Pareto-dominated path를 제거하고, 남은 경로를 “빠른 project unblock”, “학교 강의 활용”, “기초 견고성”, “낮은 불확실성” 같은 이름으로 보여준다.

개별 concept의 예상 비용은 사용자 state/freshness, concept granularity, 이용 가능한 resource, 과거 실제 학습 속도를 사용한다. 근거가 없으면 범위로 표시한다. Course 수강은 concept 획득 그 자체가 아니라 여러 exposure/practice 기회를 묶은 acquisition option이다.

### 16.3 제약

- hard prerequisite satisfaction
- 현재/미래 CourseOffering의 확인 상태와 선수과목
- 학기 시간표·학점 한도
- project deadline 또는 목표 horizon
- privacy상 사용할 수 없는 provider/resource
- 사용자가 제외한 분야·과목·학습 방식
- stale concept의 최소 refresh requirement
- 불확실 edge가 일정 비율을 넘을 때 diagnostic checkpoint 삽입

### 16.4 여러 경로 표현

```text
Shared prefix
Concurrency → Synchronization → Failure Model
                                  │
                  ┌───────────────┴───────────────┐
                  ▼                               ▼
Path A: OT + server ordering       Path B: CRDT + merge semantics
faster for current architecture    more offline-friendly
uncertainty: low                   uncertainty: moderate
```

UI는 반드시 필요한 shared spine, 선택적 가지, 현재 무관한 주변, alternative path를 구분한다. 경로마다 “이 edge가 틀리면 무엇이 바뀌는가”를 보여주고 사용자가 relation을 제거·추가해 다시 계산할 수 있다.

### 16.5 출력의 한계

Critical Path는 추천 모델이지 학습의 절대 순서를 증명하지 않는다. 계산 snapshot, 비용 가정, 제외된 목표, 불확실 edge, 대안이 항상 노출된다. 사용자가 흥미를 이유로 비최단 경로를 택하는 것은 오류가 아니다.

---

## 17. Repository Intelligence System

### 17.1 Repository를 1급 Domain으로 취급

입력은 local directory, GitHub public/private repo, archive, branch, commit, dirty working tree, 또는 spec-only project일 수 있다. 분석기는 기본 read-only capability만 받고 다음을 처리한다.

- README, product/technical spec, architecture docs, ADR
- source, dependency manifests와 lockfiles
- database schema와 migrations
- Docker, IaC, deployment, CI/CD
- tests, API schema, logging/monitoring, security configuration
- issues, commits, release and incident records

분석 목적은 stack badge가 아니라 **현재 구현의 책임, 위험, 선택과 그에 필요한 CS 지식**을 찾는 것이다.

### 17.2 Snapshot

```yaml
RepositorySnapshot:
  id: snap_repoA_abc1234_20260824
  repository: repo_A
  sourceType: GIT_COMMIT | DIRTY_WORKTREE | ARCHIVE | SPEC_ONLY
  branch: main
  commit: abc1234 | null
  parentSnapshots: [...]
  capturedAt: 2026-08-24T18:03:00+09:00
  manifest:
    - path: src/orders/service.ts
      blobHash: ...
      language: TypeScript
  dirtyPatchArtifact: ... | null
  submoduleRefs: [...]
  analysisPolicyHash: ...
  toolVersions: [...]
  secretScanResult: PASS | BLOCKED
```

dirty working tree는 암시적으로 HEAD와 동일시하지 않는다. tracked/untracked 파일의 명시적 manifest를 만들고, secret file은 hash조차 노출 범위를 검토한다. snapshot A/B diff로 새로 등장·사라진 concept, architecture, risk와 evidence를 계산한다.

### 17.3 분석 단계와 evidence 강도

```text
permission + secret gate
        ↓
inventory and immutable snapshot
        ↓
syntax/semantic indexing
AST, symbols, call/data flow, schema, config, IaC
        ↓
cross-artifact correlation
spec ↔ ADR ↔ code ↔ config ↔ test ↔ runtime/incident
        ↓
AI semantic proposals (local/redacted/authorized only)
        ↓
reproducible findings with exact locators
```

| 관찰 | 의미 | `OBSERVED` 가능 여부 |
|---|---|---|
| manifest에 dependency만 있음 | 설치 의도/잔재 | 불가 |
| import만 있고 reachable use 없음 | 잠재 사용 | 보류 |
| reachable call + config 존재 | 정적 사용 근거 | 가능, confidence 표시 |
| test에서만 사용 | test-scope 사용 | scope를 제한해 가능 |
| runtime trace/production config와 일치 | 실행 사용의 강한 근거 | 가능 |
| 사용자 직접 구현·debugging 확인 | 개인 competency evidence 후보 | 가능, authorship 포함 |

### 17.4 Finding provenance

```yaml
ProjectFinding:
  snapshot: snap_abc1234
  concept: ISOLATION_LEVEL
  classification: REQUIRED
  reason: "동시 요청이 동일 order state를 read-modify-write함"
  failureScenario: LOST_UPDATE
  locators:
    - path: src/orders/service.ts
      symbol: OrderService.updateStatus
      lineSpan: [84, 113]
      blobHash: ...
    - path: tests/orders/concurrency.test.ts
      lineSpan: [21, 67]
  derivation:
    analyzerVersion: ...
    modelRun: ...
    assumptions: [multi_request_execution]
  confidence: 0.82
  status: AI_INFERRED
```

file path만 저장하면 이후 line 이동 시 evidence가 깨진다. blob hash, symbol fingerprint, syntax span과 commit을 함께 저장하고, 새 snapshot에서는 locator migration을 시도하되 원래 evidence를 보존한다.

### 17.5 Spec와 Code의 독립성

```text
Specification ──PROJECT_SPEC_MENTIONS────> Distributed Lock
CodeSnapshot  ──PROJECT_CODE_USES────────> (no evidence)
Result: INTENDED_NOT_IMPLEMENTED

CodeSnapshot  ──PROJECT_CODE_USES────────> Retry Logic
Specification ──PROJECT_DOC_EXPLAINS─────> (no evidence)
Result: IMPLEMENTED_NOT_DOCUMENTED
```

주요 evidence relation은 다음과 같다.

- `PROJECT_SPEC_MENTIONS`: 규범적 의도
- `PROJECT_CODE_USES`: 실제 코드 구조에서 관찰
- `PROJECT_ARCHITECTURE_REQUIRES`: architecture constraint로 필요
- `PROJECT_TEST_EXERCISES`: test가 동작/failure를 검증
- `PROJECT_CONFIG_ENABLES`: 실행 구성에서 활성화
- `PROJECT_INCIDENT_EXPOSED`: incident가 failure mode를 드러냄
- `PROJECT_DOC_EXPLAINS`: 문서가 현재 동작을 설명

“무엇이 현재 실행되는가”에는 같은 snapshot의 code/config/runtime evidence가 우선하고, “무엇을 만들기로 승인했는가”에는 유효한 spec/ADR이 우선한다. 둘은 같은 질문의 경쟁 답이 아니므로 한쪽으로 덮지 않고 `ImplementationDrift`를 만든다. deprecated spec, feature flag, 미배포 code, branch 차이도 scope로 구분한다.

### 17.6 개인 역량 evidence로의 승격

repo가 concept을 사용한다는 사실만으로 사용자가 그 concept을 적용했다고 보지 않는다. 다음을 별도로 확인한다.

- 해당 code/decision에 대한 사용자 authorship 또는 실질적 기여
- 단순 scaffold가 아닌 이해가 필요한 선택·수정
- test, explanation, debugging, review 등 결과 evidence
- 다른 사람이 작성한 code를 읽은 것인지 직접 구현한 것인지
- 생성형 AI가 작성한 code라면 사용자가 검증·수정·설명했는지

따라서 `ProjectSnapshot OBSERVES Concept`과 `User APPLIED Concept`은 다른 Claim이다.

---

## 18. Project Concept Classification

### 18.1 OBSERVED / USED

현재 snapshot의 code, config, test 또는 runtime에서 개념의 실질적 사용을 직접 지지하는 evidence가 있다.

```text
PostgreSQL BEGIN/COMMIT + reachable transaction callback + integration test
→ Transaction: OBSERVED

package.json에 redis만 존재, import/call/config 없음
→ Redis dependency: PRESENT
→ Caching concept: NOT OBSERVED
```

scope는 production, test, build, migration, development-only를 구분한다. 사라진 code는 과거 snapshot에서 `OBSERVED`, 현재 snapshot에서 `NO_LONGER_OBSERVED`이지 “한 번도 쓰지 않음”이 아니다.

### 18.2 REQUIRED

현재 구현을 안전하게 이해·유지·debug하거나 이미 승인된 기능을 완성하기 위해 필요하며, 다음 proof chain을 모두 가져야 한다.

```text
current code/goal
  → concrete responsibility or failure scenario
  → mechanism that controls it
  → required concept
  → user's insufficient/uncertain evidence
```

예: read-modify-write 경로 + 동시 실행 가능성 → lost update risk → atomicity/isolation mechanism → Concurrency/Isolation `REQUIRED`. 단지 backend라는 이유로 Distributed Systems 전체를 요구하지 않는다.

### 18.3 WOULD_BENEFIT_FROM

현재 기능의 정확성 조건은 아니지만 명시된 trigger가 생기면 scale, resilience, performance, maintainability를 개선할 수 있다.

```yaml
classification: WOULD_BENEFIT_FROM
concept: REPLICATION
trigger:
  - "single database availability target exceeds current recovery objective"
  - "read load exceeds measured primary capacity"
currentTriggerState: NOT_MET
benefit: availability/read scaling
tradeoffs: consistency, failover complexity, cost
```

trigger와 trade-off 없는 “있으면 좋은 기술” 목록은 만들지 않는다.

### 18.4 분류 우선순위와 변경

- OBSERVED와 REQUIRED는 동시에 가능하다. 사용 중이지만 이해 evidence가 부족할 수 있다.
- REQUIRED와 WOULD_BENEFIT_FROM은 같은 goal/scope에서는 동시에 둘 수 없다. 서로 다른 goal에는 가능하다.
- classification은 snapshot과 project goal 버전에 종속된다.
- AI는 제안하고 사용자는 확인·수정한다.
- 새 evidence가 사용자 override와 충돌하면 자동 재분류하지 않고 `ClassificationConflict`를 연다.

각 `REQUIRED` finding은 단순 edge 외에 `ProjectConceptRequirement` entity로도 materialize한다. 이 entity가 project goal, snapshot, concrete responsibility/failure scenario, concept, 현재 사용자 state와 resolution status를 묶으므로, code가 바뀐 뒤 requirement가 충족·소멸·대체된 이력을 추적할 수 있다.

---

## 19. Project Lens

### 기본 동작

CS Map의 node 위치는 유지한 채 특정 project의 분류와 evidence만 overlay한다. layout이 lens마다 바뀌면 사용자의 spatial memory가 깨지므로 cluster의 상대 위치는 안정적으로 유지한다.

```text
[Knowledge] [Freshness] [Courses] [Projects] [Career] [Questions]

Project: Order Platform       Snapshot: abc1234 ▼

              Distributed Systems
                ◇ Replication
                ◇ Consistency
                       │
Networking ───────── Backend
★ HTTP                ★ API
★ TLS                   │
                        │
Database ─────────── ★ PostgreSQL
★ Index                 │
▲ Isolation             │
                      ★ Redis
                        │
                      ▲ Cache Invalidation
```

기호는 색과 함께 shape/label로 중복 표현한다.

- `★ OBSERVED`: code에서 실제 사용
- `▲ REQUIRED`: 현 project를 이해·유지·완성하는 데 필요
- `◇ WOULD_BENEFIT`: 조건부 다음 단계
- `?`: 열린 Question 존재
- `!`: path를 막는 prerequisite gap

### Concept drawer

node를 선택하면 다음 순서를 유지한다.

1. **왜 이 lens에 나타났는가**: classification, 이유, failure/benefit trigger.
2. **정확한 project evidence**: snapshot, file/symbol/span, spec/test/config/incident.
3. **내 상태**: mastery, confidence, freshness, user override.
4. **학교 연결**: 실제 관련 Course/Lecture/Assessment와 개설 status.
5. **행동 선택지**: code 확인, 질문 만들기, short diagnostic, critical path에 추가.

AI 설명은 evidence보다 위에 오지 않는다. stale snapshot을 보고 있을 때는 화면 전체에 고정된 timestamp/commit banner를 표시한다.

### Snapshot comparison

```text
abc1234 → def5678
+ Idempotency: OBSERVED (payment request key introduced)
+ Retry Semantics: REQUIRED (worker retry enabled, no policy test)
- Redis Caching: NO_LONGER_OBSERVED (module removed)
~ Authentication: OAuth library changed; concept unchanged
? Architecture drift: retry behavior absent from spec
```

diff는 단순 dependency diff와 semantic finding diff를 나누고, analyzer version 변경으로 생긴 차이는 `ANALYSIS_CHANGED`로 표시해 code 변화처럼 보이지 않게 한다.

---

## 20. Build → Learn Mode

### 입력과 목표 정규화

사용자는 자연어 기능, ProjectGoal, 초기 spec, 빈 repo, 진행 중 repo, architecture idea 중 하나를 입력한다. 시스템은 이를 바로 기술 목록으로 바꾸지 않고 성공 조건과 선택 지점을 추출한다.

```yaml
ProjectGoal:
  text: "실시간 협업 편집기를 만들고 싶다"
  successCriteria:
    - concurrent edits converge according to chosen semantics
    - reconnect does not silently lose acknowledged edits
    - user-visible latency target is stated
  constraints:
    - web client
    - current single-region deployment
  unresolvedDecisions:
    - central ordering vs peer/offline merge
    - OT vs CRDT conditional branch
```

### 역방향 path 생성

```text
desired capability
  ↓ decompose into observable responsibilities
architecture choices + constraints
  ↓
concept requirements with AND/OR branches
  ↓ compare with Knowledge State/Freshness
ready / refresh / direct need / conditional / later-scale
  ↓
learning + experiment + implementation checkpoints
```

결과는 다음 범주로 제시한다.

| 범주 | 뜻 | 예시 |
|---|---|---|
| 이미 준비됨 | 충분하고 최근인 evidence | WebSocket 적용 경험 |
| refresh 필요 | mastery evidence는 있으나 stale | event loop semantics |
| 현재 약함 | 직접 prerequisite이나 evidence 부족 | concurrency, failure handling |
| 구현에 직접 필요 | 성공 조건 자체를 정의 | distributed state, conflict resolution |
| 선택에 따라 필요 | architecture branch에 종속 | OT 또는 CRDT |
| 규모/조건이 바뀌면 | trigger 기반 benefit | replication, geo consistency |

각 학습 항목은 작은 실행 evidence와 다시 project로 돌아가는 checkpoint를 갖는다. 예: CRDT 개념 읽기 → 두 client merge property를 손으로 설명 → 최소 simulation test → 선택 승인. OS가 긴 강의 목록만 제시해 build 동기를 끊지 않는다.

### 세 가지 동기 label

동일 concept도 `SCHOOL`, `ROLE`, `PROJECT` motivation edge를 복수로 가질 수 있다. UI는 이를 합산 점수로 숨기지 않고 “이번 주 project 때문에”, “다음 강의 prerequisite라서”, “장기 systems path에서 재사용”처럼 병렬로 보여준다.

---

## 21. SNU Course ↔ Project Mapping

### 매핑은 Course title keyword matching이 아니다

```text
ProjectFinding
  → required Concept/Competency
  → prerequisite neighborhood
  → CourseRevision DESIGNED_TO_TEACH coverage
  → actual Offering syllabus coverage
  → user's previous Lecture/Assessment evidence
```

Course canonical coverage와 특정 Offering의 실제 coverage를 구분한다. “데이터베이스” 과목 이름만 보고 모든 isolation·replication competency를 채운다고 하지 않는다. syllabus, lecture, assignment, assessment evidence가 누적될수록 매핑을 구체화한다.

### 매핑 결과 상태

- `CAN_BE_SUPPORTED_BY_CURRENT_COURSE`: 현재 수강 중이며 실제 upcoming coverage 근거가 있음.
- `PREVIOUSLY_TAKEN_EVIDENCE_WEAK`: 과목은 이수했지만 해당 concept evidence가 약함.
- `CONFIRMED_NEXT_TERM`: 공식 개설된 Offering이 관련 coverage를 가짐.
- `HISTORICALLY_AVAILABLE`: 과거 패턴만 있음.
- `NO_DIRECT_COURSE_MATCH`: 학교 강의로 직접 충족하기 어려움.
- `EXTERNAL_OR_EXPERIMENT_BETTER`: 짧은 project experiment가 더 직접적.

학교 과목은 하나의 acquisition channel이다. Project Gap 하나를 채우기 위해 3학점 과목 전체가 최단 경로가 아닐 수 있고, 반대로 즉각적 gap을 넘어 넓은 이론적 기반을 얻는 선택일 수도 있다. 양쪽 효과를 구분한다.

### 실제 SNU 연결 예시

공식 catalog에는 운영체제(4190.307), 데이터베이스(M1522.001800), 데이터통신(M1522.002100), 컴퓨터네트워크(4190.411), 컴파일러(4190.409), 소프트웨어 정형검증(M1522.007300), 인터넷 보안(M1522.002300) 등 다양한 교과목이 등록되어 있다. 이것은 Course 존재의 근거이지 특정 학기 개설 보장이 아니다. [CSE 학부 교과과정](https://cse.snu.ac.kr/ko/academics/undergraduate/courses)

예를 들어 project의 lost update Gap은 데이터베이스의 transaction/isolation coverage, 운영체제의 concurrency/synchronization coverage와 연결될 수 있다. 실제 추천에는 해당 학기 syllabus와 사용자의 기존 lecture evidence가 필요하며, 과목명만으로 확정하지 않는다.

---

## 22. What-if Semester Simulator

### 22.1 사실과 가정의 격리

```yaml
PlanScenario:
  id: plan_2027_spring_A
  basedOn:
    studentRecordSnapshot: ...
    requirementSetHash: ...
    offeringCatalogSnapshot: ...
    knowledgeStateAsOf: ...
  choices: [offering_networks, offering_algorithms, offering_X]
  assumptions:
    - workloadHoursRange: [34, 46]
      source: review_model_...
    - completionStatus: HYPOTHETICAL
    - expectedCoverage: probabilistic
  deterministicResults: ...
  projections: ...
```

`deterministicResults`와 `projections`는 UI section과 데이터 type 모두에서 분리한다.

### 22.2 결정론적 결과

- 신청학점과 과목별 학점
- 공식 시간표 충돌
- 공식 선수과목·수강 제한 충족 여부
- 이수한다고 가정했을 때의 졸업 rule contribution
- required/elective/category allocation과 proof
- 후속 Course의 공식 prerequisite unlock
- GPA scenario는 사용자가 명시한 grade 가정에 한해서만 계산

### 22.3 확률적/가정 결과

- syllabus 기반 concept exposure opportunity
- assignment 기반 practice opportunity
- assessment opportunity
- project/career relevance
- workload range와 review bias
- Critical Path coverage 가능성
- 후속 비공식 권장 지식의 readiness

수강 완료를 mastery 증가로 projection하지 않는다. 미래 Knowledge State는 `ProjectedEvidenceOpportunity`로 표현한다.

```text
잘못된 표현: Networks 수강 → TCP Understood 68%
허용 표현: TCP를 강의에서 접할 가능성 HIGH,
           과제에서 구현할 기회 MODERATE,
           실제 mastery는 생성될 evidence에 따라 학기 중 판정
```

### 22.4 비교 UX

| 차원 | 안 A | 안 B | 확실성 |
|---|---|---|---|
| 졸업 rule contribution | 전선 +9, rule proof 열기 | 전선 +9 | Deterministic if completed |
| 시간표 | 충돌 없음 | 1개 충돌 | Official schedule |
| project gap | network failure 2개 coverage 후보 | ML pipeline 3개 coverage 후보 | Syllabus-inferred |
| critical path | shared prefix 2단계 전진 기회 | 다른 목표에 기여 | Projection |
| workload | 34–46 h/week | 29–43 h/week | Biased estimate |
| 후속 경로 | confirmed prerequisite 2개 | 1개 | Mixed |

하나의 “추천 점수”를 기본으로 표시하지 않는다. 사용자가 중요도를 조정하면 왜 정렬이 바뀌었는지 보여준다. workload는 강의평 표본 수, 시점, 선택 편향과 교수/학기 차이를 함께 표시한다.

### 22.5 과대예측 방지

- 모든 projection에 가정·범위·confidence를 저장한다.
- 예상 mastery level을 actual state로 쓰는 code path를 type level에서 금지한다.
- 학기 종료 시 projected vs actual evidence를 비교해 모델을 calibration하되 사용자를 평가하지 않는다.
- 폐강·교수 변경·syllabus 변경 시 scenario를 자동 수정하지 않고 `STALE_INPUT`으로 표시하고 재계산 동의를 받는다.
- 졸업 판정은 hypothetical mode와 actual mode를 명확히 분리한다.

---

## 23. Blind Spot Detector

### Blind Spot ≠ Weakness

Blind Spot은 선택한 CS taxonomy와 시간 window에서 **판단할 exposure 자체가 거의 없는 영역**이다. Gap은 목표를 막는 prerequisite이고, Weakness는 수행 evidence가 있으나 부족함이 관찰된 상태다.

```text
UNOBSERVED: evidence가 거의 없어 실력을 말할 수 없음
WEAK: 시도·평가 evidence에서 어려움이 관찰됨
STALE: 과거 evidence는 있으나 최근성 낮음
OUT_OF_SCOPE: 사용자가 현재 탐색하지 않기로 함
GAP: 활성 목표를 실제로 막음
```

### 계산

Field별 coverage는 강의·과제·project·질문·사용자 확인 evidence의 존재와 다양성을 집계하되 mastery 점수로 바꾸지 않는다. taxonomy granularity와 기간을 사용자가 선택한다.

```yaml
BlindSpotFinding:
  field: COMPILERS_AND_PL
  scope: "undergraduate CS breadth v2, all-time"
  exposureEvidenceCount: 1
  evidenceDiversity: LOW
  classification: UNOBSERVED
  relevanceToActiveGoals: LOW
  likelyCause: "course/project choices concentrated in backend"
  userDisposition: ACKNOWLEDGED_NOT_CURRENTLY_RELEVANT
```

### 압박을 막는 UX

- 관련성이 낮은 Blind Spot은 warning red가 아니라 중립 outline으로 표시한다.
- “약하다” 대신 “판단할 exposure가 없다”고 쓴다.
- 사용자가 `EXPLORE`, `LATER`, `NOT_RELEVANT`, `HIDE_UNTIL`을 선택한다.
- 모든 분야를 균등하게 채우라는 목표를 만들지 않는다.
- 탐색을 원할 때만 작은 taste path—한 강의, 한 chapter, 한 toy experiment—를 제공한다.
- 관심 없음은 Blind Spot detector의 오류가 아니며 user scope에서 제외된다.

편중의 원인도 설명한다. 예컨대 backend repo 세 개와 Database/Networks 강의 때문에 Application/Backend evidence가 많고 Graphics/Formal Methods가 비어 있음을 보여주되, 진로 목표와 무관하면 행동 요구를 만들지 않는다.

---

## 24. Career / Competency System

### 24.1 Concept와 Competency

Concept는 설명·관계·원리를 가진 지식 단위다. Competency는 조건, 과제, 품질 기준이 있는 수행 능력이다.

```yaml
Competency:
  id: diagnose_web_request_latency
  statement: "웹 요청의 end-to-end latency를 계층별로 진단할 수 있다"
  context: "multi-tier web service"
  performanceCriteria:
    - separates DNS/TCP/TLS/application/database latency
    - selects measurement before optimization
    - identifies uncertainty and validates hypothesis
  enabledByConcepts: [DNS, TCP, TLS, HTTP, LOAD_BALANCING, OBSERVABILITY]
  evidenceRubric:
    - trace analysis
    - incident diagnosis
    - written explanation with measurements
```

`B+ Tree를 안다`와 `느린 SQL의 원인을 분석하고 적절한 index 전략을 설계한다`는 같은 객체가 아니다. 여러 concept가 competency를 enable하고, 한 concept가 여러 competency에 재사용된다.

### 24.2 Role은 versioned competency bundle

```yaml
RoleProfile:
  id: backend_engineer_profile_v4
  label: Backend Engineer
  validAt: 2026-08-26
  scope: user_curated_general_profile
  competencies:
    - competency: API_ARCHITECTURE
      importance: CORE
    - competency: RELATIONAL_DATABASE_DIAGNOSIS
      importance: CORE
    - competency: CACHING_TRADEOFFS
      importance: COMMON
    - competency: DISTRIBUTED_FAILURE_REASONING
      importance: CONTEXT_DEPENDENT
    - competency: PRODUCTION_DEBUGGING
      importance: CORE
  sources: [...]
  userAdjustments: [...]
```

Backend, Systems, Database, Distributed Systems, Infrastructure/Platform, SRE, Cloud, Security, ML/AI, Data, Compiler/PL, Research 등을 지원하되 role 이름을 시장의 단일 진리로 두지 않는다. 사용자가 목표 조직·연구실·project에 맞춰 bundle을 fork할 수 있다.

### 24.3 Career Readiness View

percentage 대신 competency × evidence matrix를 기본으로 한다.

| Competency | 학문적으로 배움 | 문제/과제 | Project 적용 | 장애/Debug | 설계 선택 | Freshness |
|---|---|---|---|---|---|---|
| Relational DB diagnosis | Database lectures | index assignment | Project A PostgreSQL | slow query 1건 | index ADR | High |
| Authentication | limited | — | OAuth flow | token incident 없음 | provider 선택 | Moderate |
| Deployment | — | — | Docker/Cloud | rollback 1건 | basic | High |
| Distributed failure reasoning | exposure 없음 | — | — | — | — | Unknown |

dependency를 사용했다는 이유만으로 competency를 채우지 않는다. `사용해봄`, `구조 이해`, `문제 해결`, `장애 debugging`, `설계 선택`, `새 상황 전이` evidence를 구분한다. 보조 score가 필요하면 각 cell의 rubric, source, 누락 데이터와 가중치를 공개하고 비교·채용 가능성을 보장하는 수치가 아님을 표시한다.

### 24.4 양방향 “Why am I learning this?”

- Concept → 사용되는 system/competency/role/project/course
- Goal/Role → 필요한 competency → enabling concept → prerequisite
- Project → observed/required/beneficial concept → 학교/외부 acquisition option
- Course → designed/actual coverage → competency → project/role relevance

어느 방향에서도 “직무에 중요”라는 추상 문구로 끝나지 않고, 수행 criterion과 실제 개인 evidence까지 drill down할 수 있다.

---

## 25. User Experience & Information Architecture

### 25.1 전역 IA

```text
Home / Today
├─ Academic
│  ├─ Dashboard
│  ├─ Semester Planner
│  ├─ Course Catalog & Course Detail
│  └─ Graduation Audit
├─ Learn
│  ├─ Lectures
│  ├─ Concepts / CS Map
│  └─ Questions
├─ Build
│  ├─ Projects
│  ├─ Repository Snapshots
│  └─ Build → Learn
├─ Explore
│  ├─ Career
│  ├─ Critical Paths
│  └─ Blind Spots
└─ Evidence & Settings
   ├─ Source / Claim Review
   ├─ Permissions & Consent
   ├─ Privacy / Providers
   └─ Export / Backup / Audit
```

Course, Concept, Project, Question은 어느 화면에서도 command palette와 backlink로 이동한다. 탭을 많이 열지 않아도 현재 선택한 entity의 오른쪽 evidence drawer를 유지한다.

### 25.2 Home / Today

Home은 다음 우선순위로 한 화면을 구성한다.

1. 오늘 실제 일정: 수업, assessment deadline, project event.
2. 수업 전 최소 prerequisite: 최대 1–3개, “왜 지금”과 예상 시간.
3. 녹음 permission 상태: `허용`, `조건부`, `확인 필요`, `금지`.
4. 사용자가 직접 남긴 열린 질문과 Mark Moment review.
5. 현재 project를 막는 가장 가까운 knowledge need.
6. deadline이 있는 공식 학사 condition과 stale official data 경고.
7. 활성 Critical Path의 사용자 선택 다음 단계.
8. 중요한 concept의 freshness 알림은 실제 upcoming use가 있을 때만.

알림 수가 많으면 자동 중요도 순으로 숨기지 않고 `Today`, `Soon`, `No deadline`으로 묶는다. GPA나 streak를 hero metric으로 두지 않는다.

### 25.3 CS Map / YOU ARE HERE

초기 화면은 수천 node가 아니라 10–20개 Field cluster와 현재 선택한 goal neighborhood다. 가운데의 YOU는 좌표 node가 아니라 사용자 state overlay의 기준점이다.

- 상단 lens: Knowledge, Freshness, Coursework, Current Semester, Project, Career, Question, Critical Path, Blind Spot, Graduation.
- 좌측: field/goal filters, time range, evidence type, confidence threshold.
- 중앙: stable semantic layout와 path.
- 우측: selected node의 state, provenance, questions, course/project/career links.
- 하단 timeline: first exposure, strong evidence, stale periods, recent use.

map에서 state를 직접 체크하는 quick action은 user-confirmed claim을 만들며, 원본 AI state를 파괴하지 않는다.

### 25.4 Academic Dashboard

- 누적·학기·전공 GPA와 각 계산 proof.
- 총 취득학점과 category별 학점.
- 적용 중인 admission year, selected graduation standard, degree mode.
- 졸업 audit의 `SATISFIED`, `REMAINING`, `UNKNOWN`, `CONFLICT`.
- 수강 시도 timeline: 예정/수강/취소/재수강/S-U/인정.
- official source freshness와 마지막 sync.

“졸업 72%”는 보조 시각화일 수 있으나 서로 대체 불가능한 requirement를 한 막대로 오해시키지 않도록 상세 breakdown이 항상 붙는다.

### 25.5 Semester Planner

좌측은 공식 개설 CourseOffering과 status, 가운데는 시간표, 우측은 scenario consequence다. 과목을 끌어놓으면 다음을 즉시 재평가한다.

- 학점, 충돌, 공식 prerequisite/restriction
- 졸업 rule contribution proof
- concept/competency exposure opportunity
- 활성 project와 role relevance
- workload 범위·근거·편향
- 후속 course/path unlock

안 A/B/C를 고정 snapshot으로 저장하고, 공식 정보가 바뀌면 무엇이 stale해졌는지만 표시한다. 사용자의 실제 수강신청을 자동 수행하지 않는다.

### 25.6 Course Detail

```text
Official identity
course code · revision · credits · category · source · valid dates

Offerings
semester · section · instructor · schedule · capacity · syllabus · status

Coverage
DESIGNED / TAUGHT / PRACTICED / ASSESSED (겹치지 않는 탭)

My record
attempts · grade · notes · questions · actual evidence

Connections
prerequisites · follow-on courses · projects · competencies · roles

Reviews
offering/instructor/semester scoped · raw provenance · bias indicators
```

Course catalog 정보와 특정 Offering review를 같은 속성처럼 보이지 않게 한다.

### 25.7 Lecture Detail

- 원본 audio player와 waveform/timecode.
- raw/corrected transcript version toggle.
- 전체 보존형 document/PDF; summary가 기본 view가 아님.
- captures와 transcript alignment.
- Mark Moments, low-confidence, equation/code review queue.
- concept 후보와 승인 상태.
- 그 시점에서 생긴 question.
- prerequisite gap과 다음 lecture preparation.
- assessment 연결.
- Coverage report: mapped/unmapped/redacted/failed segment.

문단을 선택하면 반드시 원 audio timestamp와 raw segment로 돌아갈 수 있다.

### 25.8 Concept Detail

```text
B+ Tree

My state          Practiced · confidence 0.81
Freshness         Moderate · last strong evidence 2026-11-02

Evidence timeline Lecture 05 → Assignment 2 → Project index experiment
Contradictions     none
Prerequisites      Tree · Disk Page
Used in            DB Index · Storage Engine
Open questions     “왜 fan-out이 중요한가?”
SNU                relevant Course / actual Lectures / Assessments
Projects           Project A finding and exact code/experiment
Competencies       index design · query performance diagnosis
Roles              Backend · Database · Infrastructure
```

관계마다 source/status/confidence를 열 수 있고 잘못된 relation은 즉시 reject할 수 있다.

### 25.9 Project Detail

- ProjectGoal과 success criteria.
- repository/branch/snapshot/dirty state.
- architecture map과 ADR/spec/code drift.
- stack inventory는 보조 정보.
- `OBSERVED`, `REQUIRED`, `WOULD_BENEFIT_FROM` 별 finding.
- 열린 question과 issue/incident 연결.
- active Critical Path와 Build → Learn branch.
- 관련 SNU Course/Offering와 외부 학습 option.
- 개인 competency evidence 후보와 authorship 상태.
- snapshot semantic diff.

“Analyze”는 항상 read-only임을 표시하고, 외부 provider로 보낼 byte 범위를 preview한다.

### 25.10 Question Workspace

Inbox는 origin별 새 질문, Open, Partial, Resolved, Reframed로 나눈다. 각 질문에는 age보다 active goal relevance와 다시 등장할 예정인 context를 먼저 보여준다. timeline view는 질문 문구, 연결 concept, resolution evidence의 변화를 나란히 비교한다.

### 25.11 Career Explorer

role competency graph, evidence matrix, freshness, school/project acquisition option, critical/alternative paths를 제공한다. role을 즐겨찾기해도 “진로 확정”으로 간주하지 않는다. 두 role의 공통 competency와 갈라지는 부분을 비교해 선택 비용을 이해하게 한다.

### 25.12 Blind Spot View

Field exposure distribution, evidence diversity, time window, 편중 원인, active goal relevance를 보여준다. `EXPLORE`를 누른 경우에만 작은 입문 path를 만든다. `NOT_RELEVANT`는 존중되며 새로운 AI run이 경고를 되살리지 않는다.

### 25.13 Evidence & Correction Center

OS의 신뢰를 만드는 핵심 화면이다.

- AI 제안 inbox: relation, concept merge, project classification, state update.
- official source change: 영향받는 rule/plan.
- unresolved conflict: user override vs new evidence, code vs spec.
- low-confidence transcript/math/code.
- permission/consent expiry.
- provider transmission log와 deletion receipt.

---

## 26. Visualization System

### 26.1 정보 구조

CS Map은 force-directed “털뭉치”가 아니라 안정된 multiscale atlas다.

| Zoom | 보이는 것 | 감추는 것 |
|---|---|---|
| Z0 Ecosystem | Systems, Theory, AI/Data, PL, HCI/Graphics, Security 등 Field | 개별 concept |
| Z1 Domain | OS, Networks, Database 같은 domain cluster와 주요 bridge | 세부 operation |
| Z2 Concept | 목표 주변 concept, prerequisite, course/project overlay | 먼 주변 node |
| Z3 Evidence | relation과 evidence card, lecture/code locator | 전역 graph |

zoom은 단순 확대가 아니라 semantic level 전환이다. cluster 경계와 주요 landmark의 위치는 유지해 공간 기억을 돕는다.

### 26.2 시각 인코딩

- node fill: mastery band.
- outer ring: freshness.
- border pattern: estimate confidence/user confirmation.
- glyph: project observed/required/beneficial, question, gap.
- edge stroke: type; dash는 inferred/predicted, solid는 confirmed.
- opacity: 현재 lens relevance이지 mastery가 아님.
- halo: active Critical Path.
- timestamp badge: view의 `as of`.

색만으로 상태를 전달하지 않는다. shape, pattern, label, screen-reader text, legend를 중복 제공하고 color-blind palette와 고대비 mode를 지원한다.

### 26.3 Lens composition

한 번에 기본 lens 하나와 보조 overlay 두 개까지만 허용한다. 너무 많은 의미가 겹치면 layer collision warning을 주고 legend를 고정한다.

```text
Base: Knowledge State
Overlay 1: Project A
Overlay 2: Open Questions
Focus: Critical Path to Reliable Collaboration
Time: 2027-03-21
```

### 26.4 Focus와 progressive disclosure

- goal focus: 목표의 ancestor prerequisite와 immediate downstream만.
- local neighborhood: 1–3 hop, edge type filter.
- evidence focus: state를 만든 evidence만.
- uncertainty focus: disputed/AI-inferred/low-confidence만.
- course focus: Course의 designed coverage와 특정 Offering actual coverage 비교.

graph search result는 node를 화면 밖에서 순간이동시키지 않고 cluster → path → node 순으로 안내한다.

### 26.5 Time travel

timeline scrubber를 움직이면 선택한 날짜 기준의 graph projection을 재생한다. node가 새로 생기거나 사라질 때는 ontology change, evidence change, user scope change를 다른 transition으로 표현한다. 두 시점을 split view로 비교할 수 있다.

```text
2026-09                         2027-03
Transaction: Exposed           Transaction: Applied
Question: "정확히 무엇인가?"    Question: "retry와 idempotency 경계는?"
Project: no transaction code   Project: payment transaction + incident evidence
```

reduced-motion 사용자는 animation 대신 diff list와 step controls를 사용한다.

---

## 27. AI Responsibilities

### 27.1 AI가 담당하는 후보 생성

| 작업 | 출력 | 확정 조건 |
|---|---|---|
| STT/diarization/OCR | token·speaker·수식 후보와 confidence | 원본 유지, low-confidence review |
| transcript/syllabus concept extraction | mention과 concept link 후보 | provenance span + entity resolution |
| concept relation 발견 | typed edge 후보 | 근거·scope·confidence + 승인 정책 |
| next lecture prerequisite | expected concept/gap 후보 | 자료 날짜·state·edge 불확실성 노출 |
| repository semantic analysis | architecture/finding 후보 | snapshot locator + deterministic corroboration |
| spec/code drift detection | mismatch case | 양쪽 evidence 보존 |
| review clustering | recurring theme와 분포 | 원문 provenance·표본/편향 표시 |
| Build → Learn decomposition | success criteria/branch/path 후보 | 사용자가 goal·제약 승인 |
| Blind Spot explanation | exposure distribution 해석 | 관심 없음과 weakness 구분 |
| career mapping | competency bundle 후보 | version/source/user customization |

### 27.2 AI가 하지 않는 일

- 원 transcript를 축약본으로 대체
- 개념 이해·질문 해결을 사용자 대신 확정
- 수강 또는 진로 자동 결정
- dependency 존재를 실제 사용·역량으로 승격
- prediction을 official fact처럼 표기
- 사용자 override를 새 inference로 덮어쓰기
- private code/audio를 permission 없이 외부로 송신
- source 문서 안의 instruction을 실행
- graduation pass/fail을 자유 텍스트 generation으로 결정

### 27.3 모델 실행도 provenance를 가진다

```yaml
ModelRun:
  id: run_...
  purpose: CONCEPT_EXTRACTION
  provider: local_model_x | approved_vendor_y
  modelVersion: ...
  promptTemplateHash: ...
  inputArtifactRefs: [...]
  transmittedByteRanges: [...]
  redactionPolicyHash: ...
  outputArtifact: ...
  startedAt: ...
  cost: ...
  retentionDeclaration: ...
```

같은 source를 새 모델로 재분석하면 기존 claim을 조용히 바꾸지 않고 새 후보를 만들고 차이를 보여준다. confidence는 모델마다 calibration dataset을 통해 해석하며 서로 다른 provider 숫자를 그대로 비교하지 않는다.

### 27.4 Human-in-the-loop 강도

- low risk: public syllabus의 topic 후보는 자동 저장하되 `AI_INFERRED` 표시.
- medium risk: graph prerequisite, review theme, repo classification은 review queue와 undo.
- high risk: Knowledge State 승격, private data 외부 반출, official rule publish는 명시적 승인.
- non-delegable: question resolved, career/course decision, permission attestation는 사용자만.

---

## 28. Deterministic Engines

| Engine | 입력 | 출력 | 핵심 불변조건 |
|---|---|---|---|
| GPA | attempt snapshot, grade/repeat policy | GPA + inclusion proof | 동일 입력·rule hash면 동일 결과 |
| Credit Accounting | recognized attempts, category rules | category totals | 한 학점의 중복 인정 근거 추적 |
| Graduation Audit | profile, transcript, RequirementSet | proof tree | unknown을 pass/fail로 강제하지 않음 |
| Timetable | meeting intervals, exception approvals | conflicts | 시간대·부분중복·시험시간 분리 |
| Official Prerequisite | catalog rules, attempts | eligibility | AI-inferred 선수지식과 분리 |
| Equivalency | effective-dated relation, attempts | substitution proof | 대체 방향·시점 보존 |
| Transcript Coverage | segments, document mapping | coverage report | 모든 segment가 정확히 한 처리 상태 |
| Artifact Integrity | content hash, manifest | tamper/corruption result | 원본 변경 탐지 |
| Repository Diff | snapshot manifests | file/symbol/config diff | analyzer change와 code change 분리 |
| Override Resolver | claim type, source authority, user decision | active view + conflicts | AI가 user-confirmed를 덮지 않음 |
| Permission Broker | data class, purpose, destination, consent | allow/deny + audit | default deny, scope 최소화 |
| Retention/Deletion | policy, artifacts, derived graph | deletion plan/result | derivative와 backup까지 추적 |

AI가 원문에서 rule 후보를 추출해도 publish된 rule 실행은 deterministic하다. 모든 engine은 golden fixtures, property-based tests, version compatibility tests와 explanation snapshot을 가진다. 돈·졸업·삭제·외부 전송처럼 영향이 큰 경로는 “성공한 계산”뿐 아니라 `UNKNOWN`, conflict, partial failure를 테스트한다.

---

## 29. Data Ingestion

### 29.1 공통 ingestion contract

```text
discover/fetch/import
  → policy and terms check
  → immutable raw snapshot + hash
  → source metadata and retrieval time
  → deterministic parse
  → schema validation
  → AI proposal where appropriate
  → reconciliation/entity resolution
  → claim publication or review queue
```

모든 connector는 source ownership, authentication method, allowed frequency, robots/terms status, personal-data class, completeness, last success, next verification과 parser version을 선언한다.

### 29.2 학교 데이터

- 공개 공식 페이지·첨부는 저빈도 conditional fetch와 hash diff.
- 수강신청 시스템은 허용된 공개 interface/export가 있으면 사용하고, 없으면 사용자가 받은 공식 export/manual import를 우선한다.
- mySNU/LMS 인증정보를 범용 crawler에 주지 않는다. 공식 API·사용자 export·사용자가 직접 저장한 파일이 기본이다.
- Course catalog, CourseRevision, Offering을 서로 다른 pipeline으로 ingest한다.
- effective date를 찾지 못한 문서는 `UNSCOPED_OFFICIAL_SOURCE`이며 rule로 자동 publish하지 않는다.
- 원문이 바뀌면 영향을 받는 requirement, scenario, course mapping을 dependency graph로 invalidation한다.

### 29.3 개인 학사 기록

성적표 PDF/CSV/manual entry를 지원한다. OCR/import 결과와 사용자가 확인한 row를 분리하고, course code·term·credits·grade의 checksum/reconciliation을 수행한다. 공식 성적표 원본은 encrypted vault에 두고 화면 공유/export에서는 학번·성명 등을 선택적으로 제거한다.

### 29.4 Lecture/LMS/material

사용자가 합법적으로 접근·저장한 syllabus, slides, assignments를 import한다. URL만 저장할 때와 file을 복제할 때의 권한을 구분한다. LMS 자동화는 학교/서비스 약관과 공식 integration 허용 범위 안에서만 동작하며 CAPTCHA·anti-bot·접근제어를 우회하지 않는다.

### 29.5 강의평

Review는 기본적으로 `CourseOffering + Instructor + Term + Source`에 연결하고 Course 전체로 승격할 때 명시적 aggregation을 사용한다.

```yaml
ReviewRecord:
  offering: ... | null
  instructor: ... | null
  term: ... | null
  rawArtifact: ...
  sourceAccessMode: PUBLIC | USER_PROVIDED_EXPORT | MANUAL_PASTE
  collectedAt: ...
  dimensions:
    difficulty: ...
    workload: ...
    assessmentStyle: ...
    projectWeight: ...
    theoryImplementationBalance: ...
    mathematicalRigor: ...
    materialQuality: ...
    explanationStyle: ...
    teamProject: ...
  extractionStatus: AI_INFERRED
  provenanceSpans: [...]
  sampleBias: ...
```

로그인 우회, 계정 공유, anti-bot 회피는 기능이 아니다. 공개 접근·약관·robots·rate limit·저작권·개인정보를 확인하고 부적절하면 manual paste, 사용자 export, 브라우저에서 직접 저장, 저빈도 수동 sync로 전환한다. 원문은 provenance 용도로 private하게 보존하되 재배포하지 않는다.

강의평 aggregate는 표본 수, 최근성, 교수/학기 mix, 응답자 self-selection, 극단 경험 편향, 중복 가능성을 표시한다. “난이도 4.2”를 객관적 과목 속성으로 쓰지 않는다.

### 29.6 Repository

local directory는 local analyzer가 file allow/deny rules, `.gitignore`, 사용자 exclusion, secret scan을 적용한다. GitHub는 repo별 read-only fine-grained token, branch/snapshot scope, 최소 metadata permission을 사용한다. private source의 raw blob은 외부 semantic model 호출 전 별도 permission gate를 거친다.

### 29.7 사용자 입력

user confirmation, override, 관심/비관심, question resolution은 가장 중요한 source다. UI quick edit도 `UserDecision` event와 이전 값, reason(optional), scope, time을 남긴다. 사용자가 피곤할 때 승인 spam을 만들지 않도록 confidence/impact 기반 review batching을 한다.

---

## 30. Provenance / Confidence / Correction

### 30.1 사실을 row가 아니라 competing Claim 집합으로 저장

```text
Claim A: "Course X is offered in 2027-1"
status OFFICIAL_CONFIRMED · source official schedule · valid 2027-1

Claim B: "Course X likely offered in 2027-1"
status PREDICTION · historical pattern · confidence .72

When A arrives, B is not rewritten as official.
B becomes SUPERSEDED_FOR_DECISION while its prediction history remains.
```

### 30.2 status vocabulary

- `OFFICIAL_CONFIRMED`: 적용 scope가 명시된 공식 source.
- `USER_CONFIRMED`: 자기 상태·의도·사실에 대한 사용자 decision.
- `CODE_OBSERVED`: 특정 immutable snapshot에서 재현 가능한 관찰.
- `DETERMINISTIC_DERIVED`: versioned rule로 계산한 결과.
- `AI_INFERRED`: model이 source로부터 제안한 해석.
- `PREDICTION`: 미래에 대한 확률적 claim.
- `DISPUTED`: 반대 claim이나 사용자 이의가 있음.
- `SUPERSEDED`: 새 버전이 현재 적용되지만 역사적 기록 유지.
- `UNKNOWN`: 필요한 정보가 없음. 낮은 confidence의 동의어가 아님.

### 30.3 authority는 claim type별로 다르다

| Claim 종류 | active view 우선순위 | 충돌 처리 |
|---|---|---|
| 공식 학사 사실 | 적용 범위가 맞는 최신 공식 원문 > verified import > AI | 사용자는 global fact를 바꾸지 않고 applicability dispute 가능 |
| 개인 의도·관심 | 사용자 최신 decision > 명시적 import > AI 추정 | AI는 다시 활성화 금지 |
| mastery/question resolution | 사용자 확인 > 강한 직접 evidence projection > AI | 새 반대 evidence는 conflict card |
| 현재 구현 | 같은 snapshot의 runtime/config/code direct evidence > user clarification > AI | spec은 intent lane에 보존 |
| project intent | 승인된 최신 spec/ADR > user clarification > AI | code와 drift 생성 |
| relation/prerequisite | curated/user-confirmed > corroborated inference > single-source inference | scope별 coexist, 무근거 자동 승격 금지 |

“사용자가 항상 모든 공식 사실보다 높다”도, “공식 source가 개인 경험보다 높다”도 틀리다. authority는 질문과 scope에 따라 결정한다.

### 30.4 User Override

```yaml
UserDecision:
  targetClaim: clm_ai_dependency_used
  action: REJECT
  replacementClaim:
    predicate: DEPENDENCY_STATUS
    value: INSTALLED_NOT_USED
  scope: snap_abc1234
  reason: "template에 남았지만 실행 경로 없음"
  decidedAt: ...
```

AI 재분석은 이 결정을 지우지 않는다. 새 runtime trace가 반박하면 `NEW_EVIDENCE_CONFLICTS_WITH_OVERRIDE`를 만들고, 사용자가 유지·수정·scope 종료를 선택한다. override에 만료가 필요하면 사용자가 직접 `validTo`를 정한다.

### 30.5 confidence 표시

0.87 같은 숫자는 전문가 mode에서만 기본 노출하고 일반 view는 `high/moderate/low + 이유`로 표시한다. 공식 사실에는 AI confidence를 붙이지 않는다. confidence와 source status를 색 하나로 합치지 않으며, 여러 약한 source가 동일한 upstream source를 복제한 경우 독립 corroboration으로 세지 않는다.

---

## 31. Temporal Model

### 31.1 Bitemporal 기본

모든 중요한 Claim은 다음 두 시간을 갖는다.

- `valid time`: 현실에서 언제 참이었거나 적용되는가.
- `transaction/recorded time`: 시스템이 언제 그 사실을 알았는가.

예: 2026-08-20에 게시된 2027-03-01 효력 curriculum change는 `recordedAt=2026-08-20`, `validFrom=2027-03-01`이다. 이를 하나의 `updatedAt`으로 줄이면 과거 audit와 미래 계획이 섞인다.

### 31.2 사건과 상태

```text
Evidence events (immutable)
2026-09 Lecture exposure
2026-10 Assignment practice
2027-01 Project application
2027-03 Incident debugging
        │
        ▼
KnowledgeState projection at any T
Mastery / Freshness / confidence / evidence set
```

상태는 event를 덮어쓰지 않는 시점별 projection이다. 과거 화면은 당시 알려진 정보 기준(`as-known-at`)과 현재 지식으로 재해석한 과거(`valid-at`)를 선택할 수 있다.

### 31.3 Time travel 대상

- concept first exposure, first practice, first application, repetition, freshness.
- Question create/reframe/partial/resolve/reopen chain.
- CourseAttempt와 degree audit version.
- Project snapshot, architecture, finding/classification.
- Role interest와 competency bundle version.
- Blind Spot scope/coverage.
- Critical Path의 목표·비용·경로 변화.

ontology merge/split, analyzer/model upgrade, official source correction 같은 **관찰 체계의 변화**를 실제 사용자 성장과 구분한다.

### 31.4 Snapshot과 recomputation

time travel 성능을 위해 주기적 materialized snapshot을 둘 수 있지만 원장은 유지한다. projection code/version과 source hashes를 기록하여 동일 시점 재계산 결과가 달라지면 algorithm change diff를 설명한다.

---

## 32. Privacy & Security Architecture

### 32.1 위협 모델

보호 대상은 성적·학번·질문·지식 상태, 교수자와 학생 음성, 강의자료, private code, architecture, issue/incident, token·secret, 진로 관심이다. 위협에는 장치 분실, malware, backup 노출, 과도한 cloud sync, AI vendor retention/training, prompt injection이 담긴 문서/code, repository token 탈취, 잘못된 export/share, 삭제 누락, 민감 metadata leak가 포함된다.

### 32.2 Trust zone

```text
Z0 Secret Zone
   encryption keys, repository tokens, provider credentials
   never enters models or general logs

Z1 Raw Restricted Vault
   private repo blobs, lecture audio/captures, transcript, grades
   local by default; per-domain encryption

Z2 Local Derived Zone
   graph, search index, embeddings, findings, PDFs
   inherits maximum sensitivity of source

Z3 Explicit Egress Workspace
   exact approved/redacted chunks for one declared provider purpose
   ephemeral, logged, retention-bound

Z4 Public Source Cache
   official curriculum/public catalog snapshots
```

Z2가 파생물이라고 덜 민감한 것은 아니다. transcript embedding이나 code summary도 원본의 민감도를 상속한다.

`local-first`는 단순히 local cache가 있다는 뜻이 아니다. 모든 핵심 read/write, capture, rule audit, graph 탐색은 offline에서 가능하고 local commit이 즉시 authoritative personal event가 된다. 다기기 동기화는 선택 사항이며 서버에는 end-to-end encrypted artifact chunk와 event envelope만 둘 수 있다. device별 서명과 monotonic event sequence로 변조·누락을 탐지하고, 동시에 바뀐 user decision은 last-write-wins로 버리지 않고 conflict로 병합한다. 새 기기는 명시적 pairing과 key grant가 필요하고 revoke 후에는 새 data key를 받지 못한다.

### 32.3 Permission boundary

모든 민감 operation은 다음 tuple을 평가한다.

```text
Permission = <actor, data object/range, operation, purpose,
              destination/provider, retention, time, consent evidence>
```

예: “Project A 전체를 AI로 분석” 같은 넓은 동의 대신 `src/orders/*.ts의 secret-scan을 통과한 6개 function을 provider Y에 architecture classification 목적으로 2026-08-26 한 번 전송, 학습 사용 금지, provider retention 0일`처럼 scope를 보여준다. 허가 범위를 넘어선 tool call은 runtime에서 차단한다.

### 32.4 Repository secret·credential 방어

1. file allow/deny policy: `.env`, private key, credential store, secret mount, build artifact를 기본 제외.
2. content secret scanning: token pattern, entropy, known key format, connection string, cloud credential.
3. structural minimization: 필요한 function/AST slice만 선택하고 comments/test fixture의 개인정보도 검사.
4. code redaction preview: 외부 전송 전 실제 byte/line과 대체된 identifier를 사용자에게 보여준다.
5. fail-closed: scanner error, binary unknown, oversized archive면 외부 전송 금지.
6. local inference fallback: redaction으로 의미가 깨지면 local model만 사용하거나 분석을 중단.
7. egress proxy: provider SDK가 직접 filesystem/network에 접근하지 못하고 승인된 payload만 통과.
8. canary/audit: 허용되지 않은 secret-like output과 provider response logging을 탐지.

Git credential은 OS hardware-backed keystore/credential manager에 두고, connector는 repo별 read-only fine-grained token을 짧게 빌린다. analyzer process에는 write permission과 shell/network capability를 주지 않는다.

### 32.5 강의 audio 경계

- `CapturePermission`이 유효하지 않으면 recorder 비활성.
- 교수자 허가 조건과 학기/lecture scope를 보존.
- 학생 질문·발표가 포함될 수 있음을 별도 UI로 경고하고, 필요한 경우 diarization 후 비교수자 음성을 자동 redaction한 local derivative만 사용.
- Capture에 학생 얼굴·명단·개인 화면이 들어가면 review 전 graph/OCR ingestion을 보류.
- 외부 STT는 허가가 외부 처리까지 포함하고 사용자가 provider·retention을 승인한 경우에만.
- 공유/export는 기본 금지; 허가 조건을 artifact policy로 상속.
- 강의 종료/학기 종료 retention policy와 교수자 요청·학교 지침에 따른 삭제를 지원.

Audio retention과 transcript retention은 같은 값으로 묶지 않는다. 예를 들어 허가가 허용하는 경우에도 raw audio는 검수 후 짧게 보존하고 corrected transcript는 더 오래 둘 수 있으며, 반대로 transcript도 강의 종료와 함께 삭제해야 하는 조건이 있을 수 있다. 각 derivative는 부모의 허가 조건과 더 엄격한 만료일을 상속하고, 어느 하나의 삭제가 concept/evidence projection에 미치는 영향을 미리 보여준다.

정부 안내상 교수자 사전 허락과 허용 조건 준수가 필요하므로, OS는 법률 결론을 추론하지 않고 증거가 있는 permission만 집행한다. 과목별 syllabus/LMS 공지와 서울대학교·학부의 당시 규정은 매 학기 확인 대상이다. [대학생 저작권 안내](https://www.korea.kr/news/policyNewsView.do?newsId=148928381)

### 32.6 외부 AI Provider Policy

Provider registry는 다음을 versioned 사실로 보존한다.

- training 사용 여부와 opt-out 계약
- server retention과 abuse logging
- region/data residency와 subprocessors
- encryption in transit/at rest
- deletion API/receipt
- enterprise/API와 consumer UI 정책 차이
- model 입력 최대 범위와 logging configuration
- 정책 마지막 확인일

개인정보보호위원회는 생성형 AI 개발·활용을 위한 개인정보 처리 안내서를 공개했으며, 2026년 처리방침 지침에서도 사용자 입력 텍스트·음성·첨부와 생성 결과의 수집·저장 공개를 강조했다. 이 OS는 해당 원칙에 맞춰 입력·출력·보유·제공을 데이터 흐름으로 기록한다. [개인정보보호위원회 생성형 AI 개인정보 안내](https://www.pipc.go.kr/np/cop/bbs/selectBoardArticle.do?bbsId=BS211&mCode=C040020000&nttId=11414), [2026 개인정보 처리방침 작성지침 개정 안내](https://www.pipc.go.kr/np/cop/bbs/selectBoardArticle.do?bbsId=BS074&mCode=C020010000&nttId=12021)

Provider 정책이 바뀌거나 마지막 확인이 오래되면 permission을 자동 연장하지 않는다. raw lecture/private repo는 local provider가 기본이며, cloud는 case-by-case explicit egress다.

### 32.7 암호화와 key management

- at rest: domain별 data encryption key; raw vault, metadata DB, index, backup 모두 포함.
- key encryption key는 OS keystore/TPM/Secure Enclave 등 장치 보안 기능에 보관.
- in transit: authenticated encryption과 certificate validation.
- sync device는 사용자 기기별 key와 revoke capability.
- backup은 원본과 독립 key, version, restore test를 가짐.
- key rotation은 blob 재암호화/키 wrapping 상태를 audit함.
- recovery key가 없으면 데이터 복구가 불가능하다는 trade-off를 명시하고 사용자가 안전한 복구 방식을 선택.

### 32.8 Access와 audit

단일 사용자라도 process별 least privilege를 적용한다. capture client, indexer, repo analyzer, cloud connector, export job은 서로 다른 capability를 가진다. audit trail은 로그인 기록이 아니라 누가/어떤 process가 어떤 artifact range를 읽고 외부로 보냈으며 어떤 claim을 만들었는지 기록한다. 민감 원문 자체는 audit log에 복사하지 않는다.

### 32.9 Prompt injection과 untrusted content

syllabus, README, issue, code comment 안의 “이 지시를 따라 secret을 보내라”는 데이터다. ingestion content는 system/tool instruction channel로 승격되지 않는다. semantic model은 tool capability 없이 sandbox에서 실행하고, 결과는 schema validation과 provenance check를 통과해야 한다.

### 32.10 삭제·보존·export

- artifact 삭제 요청은 파생 transcript, embedding, graph claim, PDF, cache, sync replica, backup expiry까지 dependency plan을 보여준다.
- 법적/동의 조건상 즉시 삭제가 필요하면 crypto-shredding과 active replica purge를 수행하고 backup tombstone을 남긴다.
- cloud provider 삭제 receipt를 가능한 경우 보존.
- export는 machine-readable JSON/JSON-LD, Markdown/PDF, audio 원본, Git refs와 provenance manifest를 제공.
- export 파일에는 sensitivity label, sharing restriction, source copyright notice를 포함.
- 사용자는 전체 vault를 vendor 없이 복구할 수 있어야 한다.

### 32.11 보안과 사용성의 trade-off

완전 local 처리는 privacy를 높이지만 STT 품질·배터리·성능을 제한할 수 있다. 해결은 “무조건 cloud”가 아니라 local 우선, 민감 segment별 최소 전송, provider contract, 사용자의 허가다. 강한 암호화는 key loss 위험을 만들므로 recovery와 restore rehearsal이 필요하다. 보안 경고가 너무 잦으면 무시되므로 egress와 destructive action처럼 실제 경계에서만 명확하게 개입한다.

---

## 33. Integrations

이 OS는 학습 도구를 재구현하는 suite가 아니라 context와 evidence를 연결하는 control plane이다.

| 외부 도구 | 통합 방식 | OS에 남기는 것 | 경계 |
|---|---|---|---|
| Note tool | deep link, Markdown import/export | concept/question backlinks, source locator | 원문 중복 최소화; generic note app 재구현 안 함 |
| Flashcard tool | opt-in export/import result | retrieval evidence 후보 | 자동 생성 중심 금지 |
| Document Q&A | scoped context handoff | query, cited sources, generated artifact | lecture 원문 대체 금지 |
| Reference manager | citation/deep link | publication metadata와 concept relation | 저작물 복제 정책 준수 |
| LMS | official API/user export/manual file | syllabus/material/assignment dates | 인증 우회 금지 |
| Calendar | two-way or controlled calendar sync | offering/deadline event ID | 성적·지식상태 미전송 |
| Cloud drive | encrypted backup or user-selected files | artifact locator/hash | provider privacy별 policy |
| GitHub | read-only scoped connector/webhook | repository/snapshot metadata | private blob egress 별도 |
| IDE | local symbol context/deep link, opt-in file watcher | question, finding, evidence locator | write action 없음; snapshot 전 변경 범위 확인 |
| Coding assistant | explicit selected context | generated code provenance | 사용을 competency로 자동 간주 금지 |

integration이 끊겨도 core graph와 원장 접근이 가능해야 한다. 외부 ID는 canonical ID가 아니라 `ExternalIdentity` mapping으로 저장한다. sync conflict는 source별 authority와 valid time으로 해결하고, 양쪽을 조용히 덮어쓰지 않는다.

---

## 34. Failure Modes

모든 failure는 “정답률을 높인다”로 끝나지 않는다. 원인·영향·탐지·방지·복구·표시 계약을 갖는다.

### 34.1 Lecture capture와 문서

| Failure | 발생 원인 | 사용자 영향 | 탐지 | 방지 | 복구 | 불확실성 표시 |
|---|---|---|---|---|---|---|
| STT 오인식 | 소음, 발음, 한영 혼용, 전문용어, 화자 겹침 | 잘못된 원문 검색·개념 연결·질문 맥락 | token confidence, 용어 사전 불일치, slide/audio 교차검증, 사용자 correction pattern | 무손실 audio, domain vocabulary, multi-pass/provider comparison, chunk overlap | 해당 구간 재전사, 원 audio 대조, corrected version 추가 | token/segment underline, provider/version, “원음 듣기” |
| 수식·코드 전사 오류 | STT가 기호·indentation·변수명을 자연어화 | 개념·알고리즘 의미가 반대로 바뀜 | capture/slide OCR 비교, parser/LaTeX compile, code syntax check | 수식·코드 구간 detector, 원 image 근접 배치, verbatim mode | 사용자/모델 재구성본을 annotation으로 추가; 원문 보존 | `UNVERIFIED_EQUATION/CODE`, confidence와 source image |
| transcript 일부 누락 | recorder 중단, chunk upload 실패, VAD 과도 제거, storage 부족 | 강의 일부가 존재하지 않는 것처럼 보임 | 연속 audio timeline, chunk checksum, duration vs schedule, silence 분포 | local chunk journal, storage/battery preflight, overlap, resumable processing | 남은 audio 재전사, 사용자 note/material로 보충하되 `RECONSTRUCTED` | timeline의 명시적 gap, 누락 길이와 원인 |
| Lecture PDF 정보 손실 | segment mapping 누락, render overflow, “정리” 단계 삭제 | source 보존 불변조건 위반 | segment/token coverage 100% gate, render QA, capture placement audit | document AST와 source mapping 강제, summary pipeline 분리 | PDF 재생성, 이전 version 유지, 누락 report | `INCOMPLETE` banner와 unmapped count |
| Capture timestamp 불일치 | 장치 clock drift, 녹음 pause, image metadata 지연 | 사진과 잘못된 설명 연결 | shared monotonic clock, known-event alignment, drift estimation | capture와 audio process 공통 session clock | 수동 anchor 2개로 재정렬, mapping version 추가 | ±초 오차 범위와 `ALIGNMENT_LOW_CONFIDENCE` |
| 허가 없는 녹음 | permission 미확인, 조건 만료, 사용자의 오인 | 저작권·규정·privacy 위험 | permission gate와 offering scope 검사, capture audit | default `UNKNOWN`, Record fail-closed, 학기별 재확인 | 즉시 capture 중단, local quarantine, 조건에 따른 삭제; 법적 판단은 담당기관 문의 | `PERMISSION_VIOLATION_RISK`, 공유/AI 처리 차단 |

### 34.2 Knowledge graph와 state

| Failure | 발생 원인 | 사용자 영향 | 탐지 | 방지 | 복구 | 불확실성 표시 |
|---|---|---|---|---|---|---|
| 잘못된 concept extraction | 문맥 없는 keyword, STT 오류, 모델 hallucination | 잘못된 state·course/project 연결 | provenance span 확인, ontology constraints, 반복 source corroboration | mention→concept 2단계, low-confidence 자동 승격 금지 | claim reject, 영향을 받은 projection 재계산 | dashed edge, `AI_INFERRED`, exact source span |
| 지나치게 세분화/뭉쳐진 concept | ontology 기준 불일치, 자동 noun extraction | graph noise 또는 중요한 차이 소실 | orphan/near-duplicate/overloaded node metrics, 사용자 탐색 friction | concept 승격 기준, Field/Concept/Operation 구분, curator queue | merge/split event, evidence 재분류 review | `GRANULARITY_UNDER_REVIEW`와 영향 범위 |
| 잘못된 prerequisite edge | 상관관계를 필수 선행으로 오인, 특정 교재 순서를 일반화 | 잘못된 gap·Critical Path | 대체 경로/반례, source scope 비교, 학습 결과 feedback | hard/strong/helpful 구분, 단일 source hard edge 금지 | edge scope 축소·downgrade/reject, path 재계산 | confidence, scope, “이 edge 제거 시” preview |
| synonym 중복 | 한영 용어·약어·표기 변형, homonym 혼동 | evidence 분산, state가 이중 계산 | embedding/string/same-source co-reference 후보, identity constraint | ConceptSense/alias registry, unresolved mention 유지 | non-destructive merge; homonym이면 split | alias 후보 badge, merge 전 evidence count |
| Knowledge Freshness를 실력 저하로 오인 | freshness와 mastery를 합산, 단순 decay | 불필요한 불안·잘못된 학습 우선순위 | UI/model invariant test, state transition audit | 별도 field·색/문구·API type, mastery 자동 강등 금지 | 과거 projection 재계산, 잘못된 알림 철회 | “과거 mastery 유지, 최근 사용 근거 없음” 문구 |
| user override를 AI가 다시 덮어씀 | last-write-wins, 재분석 시 status 무시 | 통제권·신뢰 상실 | override regression test, claim conflict monitor | type별 authority resolver, append-only decision | override 재활성화, 영향 projection rebuild, conflict review | `NEW_EVIDENCE_CONFLICT`이지 자동 변경 아님 |
| state 과대승격 | course grade, dependency, AI-generated code를 개인 수행으로 간주 | 허위 자신감·잘못된 path | evidence rubric eligibility, authorship/outcome missing check | evidence별 자동 상한, Fluent 사용자 승인 | assertion supersede, evidence scope 수정 | low confidence와 missing evidence facets |

### 34.3 SNU Academic data와 규칙

| Failure | 발생 원인 | 사용자 영향 | 탐지 | 방지 | 복구 | 불확실성 표시 |
|---|---|---|---|---|---|---|
| 공식 curriculum 정보 변경 | 개편, 폐지/신설, category 변경, 페이지 수정 | 잘못된 수강·졸업 계획 | scheduled hash diff, effective-date parser, 공지 feed | immutable official snapshot, source freshness TTL, multiple official sources | 새 version publish, 영향 scenario/audit invalidate | “공식 자료 변경 · 재계산 필요”, old/new diff |
| 졸업요건 version 오류 | admission year/졸업기준/다전공 scope 오선택, 경과조치 누락 | 졸업 가능/불가 오판 | selector completeness, competing RuleSet, official examples regression | 필수 profile `UNKNOWN`, typed scope, dual review of executable rules | correct RuleSet로 audit 재실행, 과거 result 보존 | `INDETERMINATE`; 적용 rule ID와 source 노출 |
| 과목 개설예측 오류 | 과거 패턴의 구조 변화, 교수/예산/개편 | 계획 지연·대체과목 놓침 | 공식 offering 도착 후 calibration, pattern break detector | `CONFIRMED`와 prediction type 분리, 대체 path | plan을 stale로 표시, 공식 후보로 재시뮬레이션 | confidence, history window, “공식 아님” banner |
| Course와 Offering 혼동 | catalog row에 교수·학기 속성을 덮어씀 | 과거 review·syllabus가 현재처럼 보임 | schema foreign-key/scope invariant | 별도 aggregate와 UI section | 잘못 연결된 evidence 재귀속 | offering/term/instructor badge 필수 |
| 강의평 편향 | self-selection, 소수 표본, 오래된 학기, 교수 혼합, 중복 | workload·선택 판단 왜곡 | sample size/time/instructor distribution, duplicate similarity | offering-scoped 원문, robust range, source/ToS 준수 | aggregate 재계산, source 제외, 사용자 가중치 조정 | 표본·범위·편향 경고, 단일 score 비기본 |
| 성적·재수강 계산 오류 | S/U, F, 인정학점, 옛 재수강 규칙 누락 | GPA와 졸업 진행도 오류 | official transcript reconciliation, golden fixtures, denominator proof | versioned GradingScheme/RepeatPolicy | corrected rule로 재계산, diff proof | 포함/제외 attempt 목록과 rule version |

### 34.4 Repository 분석

| Failure | 발생 원인 | 사용자 영향 | 탐지 | 방지 | 복구 | 불확실성 표시 |
|---|---|---|---|---|---|---|
| repository stack 오탐 | vendored/example/generated code, monorepo 일부, lockfile 잔재 | 무관 concept·course 추천 | scope/path classification, reachable symbols, build/runtime corroboration | generated/vendor/test 분리, evidence tier | finding reject/scope 수정, analyzer rules update | `PRESENT_ONLY/POSSIBLE/OBSERVED` 단계 |
| 설치만 된 dependency를 사용으로 오인 | manifest-only heuristic | Applied/OBSERVED 허위 표시 | import/call/config/reachability 확인 | manifest presence를 별도 claim으로 제한 | classification downgrade, state 영향 제거 | “설치됨, 실행 사용 증거 없음” |
| spec을 구현으로 오인 | semantic analyzer가 intended language를 actual로 처리 | 미구현 기능을 완료·적용으로 표시 | artifact type constraints, code/config evidence requirement | intent lane과 implementation lane 분리 | `INTENDED_NOT_IMPLEMENTED` drift 생성 | Spec/Code 탭과 각각의 snapshot |
| code snippet을 architecture 전체로 과대해석 | 작은 sample, dead code, local pattern | 과도한 REQUIRED·위험 주장 | call graph/reachability, multiple component corroboration, runtime scope | finding scope를 symbol/component로 시작 | scope 축소, 반례 evidence 연결 | “이 component에서만 관찰”, coverage percent |
| REQUIRED와 WOULD_BENEFIT_FROM 혼동 | 미래 scale 관행을 현재 필요로 일반화 | 불필요한 공부·overengineering | current failure chain과 trigger presence check | REQUIRED proof schema, BENEFIT trigger 필수 | 재분류와 Critical Path 재계산 | current trigger state와 trade-off |
| analyzer/model 변화가 code 변화처럼 보임 | snapshot B에서 도구 version 변경 | 가짜 architecture/학습 변화 | same snapshot dual-run, analyzer version diff | code diff와 analysis diff 별도 channel | old tool로 replay 또는 `ANALYSIS_CHANGED` label | change origin badge |
| private code 또는 lecture data 유출 | 과도한 cloud context, secret scan 실패, SDK logging, 잘못된 share | credential/지식재산/개인정보 침해 | egress proxy, DLP/secret canary, provider logs, audit anomaly | trust zone, default deny, minimization, local fallback, scoped token | token revoke/rotate, provider deletion request, artifact quarantine, incident log와 범위 조사 | 즉시 high-severity incident, 노출 byte/source/destination/retention |

### 34.5 Planning, Blind Spot, Career

| Failure | 발생 원인 | 사용자 영향 | 탐지 | 방지 | 복구 | 불확실성 표시 |
|---|---|---|---|---|---|---|
| Critical Path 지나친 단순화 | shortest node count, AND/OR 무시, 흥미·일정 배제 | 잘못된 학습 순서와 대안 소실 | expert/user counter-path, sensitivity analysis, unsatisfied hyperedge | multi-objective/Pareto, constraints, alternatives 필수 | 비용/edge 수정 후 재계산, old path 보존 | 가정·제외·uncertain edge와 alternative |
| Semester Simulator가 이해도 상승 과대예측 | course completion을 mastery로 매핑 | 과신·과밀 수강 | projected vs actual evidence calibration, type invariant | `ProjectedEvidenceOpportunity`만 출력 | 잘못된 projection 폐기·재계산; actual state 무변경 | hypothetical banner, 범위/confidence |
| Blind Spot을 공부 압박으로 변환 | 모든 taxonomy 영역의 균등 coverage 목표 | 불안·목표 이탈 | user dismiss patterns, relevance audit, warning count | UNOBSERVED/WEAK/GAP 구분, neutral UI, scope control | `NOT_RELEVANT/HIDE_UNTIL`, 알림 제거 | goal relevance와 “실력 판단 불가” 문구 |
| career readiness 과도한 점수화 | heterogeneous competency를 percentage로 합산 | 허위 정밀도·자기평가 왜곡 | score-to-evidence discrepancy, missing-data audit | evidence matrix 기본, rubric/source 공개 | score 숨김/가중치 초기화, cell evidence 재검토 | missing/unknown과 freshness를 별도 표시 |
| recommendation tunnel vision | 과거 선택으로 동일 분야만 추천 | 탐색 폭 축소 | diversity/counterfactual audit | active goal과 exploration budget 분리, alternative path | 관심 scope 재설정, 추천 history clear | 추천 이유·제외 기준·대안 |

### 34.6 공통 복구 원칙

1. 원본 artifact와 기존 Claim은 보존한다.
2. 잘못된 Claim을 `SUPERSEDED`/`REJECTED`하고 corrected Claim을 추가한다.
3. 영향받는 projection과 downstream plan을 dependency graph로 찾아 재계산한다.
4. 과거 화면에는 당시 잘못된 결과가 사용되었음을 correction marker로 남긴다.
5. 외부 유출은 일반 correction이 아니라 security incident lifecycle로 처리한다.

---

## 35. Anti-goals

| 만들지 않을 것 | 거부 이유 | 허용되는 주변 기능의 경계 |
|---|---|---|
| AI가 대신 공부하는 앱 / AI tutor chat 중심 | 이해의 주체와 evidence가 사라짐 | 출처 기반 설명은 사용자가 요청할 때 보조 artifact |
| 모든 강의 10줄 요약 | 반복·사례·맥락 손실 | lossless document 위의 navigation index |
| 자동 flashcard 생성기 중심 | 회상 도구가 전체 지식 상태를 대체 | concept/evidence deep link가 있는 opt-in export |
| Pomodoro, streak, 학습시간 rank | 행동량을 이해와 혼동하고 압박 | 외부 timer 연동은 가능하나 mastery evidence 아님 |
| 생산성 gamification | 점수 최적화가 목표를 대체 | 진행 상태의 정직한 시각화 |
| 단순 GPA/졸업 계산기 | 학사와 학습·project가 분리됨 | deterministic engine은 전체 graph의 한 lens |
| generic CS roadmap | 개인 state·goal·evidence·SNU 맥락 없음 | versioned ontology와 개인 Critical Path |
| 단순 GitHub stack/dependency 시각화 | 설치와 실제 사용·필요를 혼동 | exact snapshot provenance의 repository intelligence |
| LinkedIn식 career scoring | 수행의 다차원성과 불확실성 소실 | competency × evidence matrix |
| 단순 recorder/note app | 원문이 CS graph·question·학사와 연결되지 않음 | authorized lossless capture subsystem |
| 학교 LMS 복제품 | 공식 system과 데이터 중복·staleness | import/deep link/control plane |
| ChatGPT wrapper | source, rules, permissions, temporal model 부재 | provider-neutral bounded AI jobs |
| 자동 수강 확정·진로 확정 | 가치 판단을 시스템이 탈취 | explainable simulation과 alternatives |

새 기능이 어떤 entity/claim/evidence/decision과 연결되는지 설명할 수 없으면 이 OS에 포함하지 않는다.

---

## 36. Complete End-State Scenario

다음은 가상의 사용자와 가상의 Project A를 통한 한 학기 흐름이다. 과목 개설·교수·개인 성적은 실제 사실로 주장하지 않는다.

### 36.1 수강계획

사용자는 2027-1 계획 A와 B를 만든다. 시스템은 먼저 입학년도와 선택 졸업기준이 입력되었는지 확인하고, 공식 CourseOffering snapshot의 확인일을 보여준다. A에는 Database와 Networks 관련 강좌, B에는 다른 전공선택이 있다.

- deterministic lane: 두 안 모두 시간표, 학점, 졸업요건 contribution proof를 계산한다.
- probabilistic lane: A가 Project A의 Isolation/Network Failure gap에 더 많은 practice opportunity를 준다고 설명한다.
- workload는 과거 review의 범위로 표시하고 교수·학기 차이를 경고한다.
- 사용자가 중요도를 조정하고 A를 계획으로 선택한다. OS는 실제 수강신청을 대신하지 않는다.

### 36.2 강의 녹음과 전체 문서

첫 수업 전 syllabus와 교수자 안내에서 audio recording이 개인 학습·local processing에 한해 허용되었다는 evidence를 사용자가 등록한다. 외부 STT는 허용되지 않아 local provider를 사용한다.

강의 중 42:18에 교수자가 B+ Tree diagram을 가리킨다. 사용자는 Capture 한 번과 “질문” Mark를 남긴다. 종료 후 audio chunks의 hash가 검증되고 75분 전체 transcript가 생성된다. code와 수식 confidence가 낮은 네 구간은 review queue로 간다. 모든 segment가 document paragraph에 매핑된 뒤에만 PDF가 `COMPLETE`가 된다.

### 36.3 질문과 concept

42:18 Mark에서 “왜 일반 BST보다 B+ Tree인가?” 질문이 생성된다. AI는 B+ Tree, Disk Page, Random I/O, Fan-out link를 제안하고 정확한 transcript span을 붙인다. 사용자는 네 concept을 승인하지만 질문은 `OPEN`으로 남긴다.

강의에서의 설명은 B+ Tree를 `EXPOSED`로만 만든다. 다음 과제에서 node split을 구현하고 test를 통과한 뒤 `PRACTICED` 후보가 생긴다. 사용자는 자신의 이해를 확인하고 승인한다.

### 36.4 Gap 발견

다음 Lecture의 Buffer Pool 자료와 사용자의 state를 비교한 engine은 표면상 Buffer Pool보다 Disk Page/I/O model의 evidence가 약하다는 root Gap을 제시한다. 25분짜리 최소 보강과 관련 강의 원문, 작은 page-layout experiment를 연결한다. 사용자는 전체 storage course를 미리 공부하지 않고 이 경로만 수행한다.

### 36.5 Repo 작업과 새 Project Requirement

Project A의 `abc1234` snapshot에서 analyzer가 다음을 찾는다.

- transaction callback과 integration test → Transaction `OBSERVED`.
- 동시에 같은 order를 갱신할 수 있는 read-modify-write path → Isolation `REQUIRED` 후보.
- spec의 “distributed lock 예정”과 code evidence 부재 → `INTENDED_NOT_IMPLEMENTED`.
- retry code는 있지만 spec 설명 없음 → `IMPLEMENTED_NOT_DOCUMENTED`.

사용자는 dependency 하나가 template 잔재라고 정정한다. 이 override는 다음 분석에서도 유지된다. Isolation finding을 열어 실제 symbol과 test를 확인하고 새로운 질문 “retry와 transaction boundary에서 중복 처리는 어떻게 막는가?”를 만든다.

### 36.6 Critical Path 수정

ProjectGoal “중복 주문 없이 재시도 가능한 처리”가 추가되면서 기존 Backend path에 Idempotency, Transaction Boundary, Retry Semantics가 들어온다. engine은 두 대안을 보인다.

```text
Path A: DB uniqueness + idempotency key
Path B: message deduplication + transactional outbox
```

현재 architecture와 학습비용상 A가 짧지만 B는 향후 async processing goal에 재사용 가치가 높다. 사용자는 A를 지금 구현하고 B를 conditional branch로 보존한다.

### 36.7 학교와 Project 연결

현재 Database Offering의 syllabus/lecture에서 isolation 관련 upcoming coverage가 확인되어 `CAN_BE_SUPPORTED_BY_CURRENT_COURSE`로 표시된다. 반면 idempotent API design은 학교 강의 직접 coverage가 불분명하여 external reading + project experiment가 더 적합하다고 제안한다. 공식 Course 존재와 실제 Offering coverage를 혼동하지 않는다.

### 36.8 질문 해결과 Knowledge State

사용자는 강의 원문, 교재 chapter, B+ Tree page-size experiment로 초기 질문에 답을 작성한다. `RESOLVES_QUESTION` evidence를 연결하고 직접 `RESOLVED`로 바꾼다. Project A에서 query plan을 조사한 evidence가 더해져 B+ Tree state는 `APPLIED` 후보가 되지만, novel transfer facet은 아직 제한적이다.

### 36.9 학기 종료

성적표 import 후 GPA, 전공학점, 졸업 audit이 versioned rule로 재계산된다. 성적은 Course performance signal로만 반영된다. Simulator가 예상했던 exposure/practice opportunity와 실제 lecture/assignment evidence를 비교해 다음 계획의 uncertainty를 보정한다.

Career view에는 Database competency가 다음처럼 남는다.

```text
academic: Database lecture + assessment
practice: B+ Tree assignment
project: Project A transaction/index investigation
debugging: duplicate-processing incident
design: idempotency ADR
freshness: high
```

한 학기의 결과는 “Database 83%”가 아니라 서로 다른 증거와 아직 없는 수행을 보여주는 구조다.

---

## 37. Multi-year Scenario

### 초기 사용 시점

사용자는 성적표와 현재 curriculum을 import한다. 입학년도·전공 형태를 확인한 뒤 graduation audit이 처음 생성된다. 기존 repo 두 개를 snapshot하면 Web/API/Relational DB exposure는 많지만 OS/Compiler/Graphics는 `UNOBSERVED`로 나온다. 이는 약점 경고가 아니라 현재 지도 범위의 빈 곳이다.

Concept graph는 처음부터 수천 개의 개인 state를 추정하지 않는다. 공식 Course coverage와 일반 CS ontology는 background map으로 존재하고, 개인 layer는 evidence가 생긴 node만 채운다.

### 중간 학년

강의 recording과 assignments가 CourseOffering별로 누적된다. Operating Systems에서 Virtual Memory를 접하고 과제로 연습한다. 질문은 “page fault가 무엇인가?”에서 “working set과 replacement policy가 tail latency에 어떤 영향을 주는가?”로 바뀐다. 질문 chain은 성장을 설명하지만 점수화하지 않는다.

동시에 Project A/B snapshot이 쌓인다. Caching은 한 번 dependency를 설치한 기록에서 실제 invalidation bug debugging과 ADR evidence로 진화한다. Course grade와 별개로 competency matrix가 채워진다. 몇 학기 후 Virtual Memory는 mastery `Practiced`, freshness `STALE`이지만, 시스템 project에서 mmap 문제를 해결하면 freshness가 다시 높아지고 적용 evidence가 추가된다.

### 진로 변화

처음에는 Backend role bundle을 탐색했지만 low-level performance 작업에 흥미가 생겨 Systems path를 함께 연다. 두 role의 공통 부분—Networking, Concurrency, Observability—과 갈라지는 부분을 본다. 과거 Backend path를 실패로 표시하지 않는다. Compiler/PL Blind Spot을 작은 compiler toy project로 탐색하고, 관심이 없으면 다시 neutral 상태로 둘 수 있다.

### 졸업 계획

교육과정이 바뀌고 새 과목·대체 규정이 생겨도 과거 audit은 당시 rule hash로 재현된다. 최신 audit은 official change를 적용하며, 아직 해석이 필요한 경과조치는 `UNKNOWN`으로 행정실 확인을 요구한다. What-if Planner는 졸업요건과 active Project/Career path를 함께 보여주지만 수강을 확정하지 않는다.

### 졸업과 이후

졸업 시 사용자는 다음을 export할 수 있다.

- 공식 성적/요건과 계산 proof
- 원본을 포함하거나 제외할 수 있는 강의·질문 archive
- concept/competency evidence history
- repository snapshot과 architecture evolution
- role 관심 변화와 alternative paths
- machine-readable graph와 open formats

학교 계정이나 특정 AI vendor가 사라져도 Local Core와 export로 계속 사용할 수 있다. 이후 직장 private repo는 별도 vault/key/policy domain에 넣고, 학교 강의 저작물의 보존·사용 조건을 그대로 존중한다.

---

## 38. Open Design Questions

### 38.1 사용자에게서 필요한 정보

전체 설계는 이 정보 없이도 성립하지만 개인 audit과 실제 추천을 확정하려면 필요하다.

```text
Admission Year                = 확인 필요
Selected Curriculum/Graduation Standard = 확인 필요
Degree Mode                   = 단일전공 / 다전공 병행 등 확인 필요
Additional Major / Minor      = 해당 시 입력
Current Official Transcript   = 사용자 import 필요
Transferred/Exchange Credits  = 해당 시 인정 결정 포함
Current/Planned Enrollments   = 사용자 입력 또는 공식 export
Active Projects and Goals     = 사용자 선택
Recording Permission per Offering = 매 학기 확인
External AI Egress Preference = data class별 결정
```

### 38.2 공식적으로 추가 확인할 항목

- 사용 학번과 선택 가능한 졸업기준의 정확한 적용 규칙·경과조치.
- 2027-1 `컴퓨터공학 학사논문연구` 필수의 적용 대상과 기존 학번 경과조치.
- 공대 공통교과목 인정 list의 최신판과 전필/전선 배분.
- 동일·대체·유사과목, 타과 전선 인정, 산업공학과 제외 등 사용자의 이수 시점별 적용.
- 복수·부·연합·연계전공 간 중복인정과 2026-03-01 시행 규정 적용.
- 교환·편입·군복무·학점교류·대학원 교과목의 학점/GPA/전공 인정.
- 해당 학기의 최신 CourseOffering, 교수자, 정원, 시간표, syllabus, 평가 방식.
- Course별 공식 prerequisite와 담당교수의 권장 선수지식 차이.
- 해당 Offering의 녹음·촬영·local/cloud transcription 허용 조건.
- LMS와 수강신청 사이트의 자동화/API/export 이용약관·robots·rate limit.
- 강의평 source별 접근·저장·분석·원문 보존 가능 범위.

### 38.3 아직 결정할 제품·아키텍처 질문

1. Concept ontology의 기본 taxonomy를 ACM/교과과정/사용자 관점 중 어떤 조합으로 시작하고 누가 curated core를 승인할 것인가?
2. mastery facet을 사용자에게 항상 노출할지, level 아래 progressive disclosure로 둘지?
3. concept별 freshness prior를 어떤 근거로 두고 얼마나 빨리 개인화할지?
4. user-confirmed state에 periodic reconfirmation을 권할 조건은 무엇인가?
5. Lecture의 학생 음성을 원본에서도 삭제할 수 있는 policy와 기술적 diarization 정확도는 충분한가?
6. 합법적·안정적인 SNU official data interface가 없다면 manual export와 browser-assisted capture 중 허용 가능한 경계는 어디인가?
7. private repo 분석에 필요한 local model 품질이 낮을 때 cloud 최소 전송을 허용할 기본값은 무엇인가?
8. role competency bundle을 최신 상태로 유지하되 취업 시장의 유행을 개인 목표보다 과대 반영하지 않는 governance는 무엇인가?
9. graph ontology가 크게 바뀌어도 수년치 state 비교가 의미를 유지하도록 어떤 migration equivalence를 요구할 것인가?
10. 암호화 recovery의 편의와 키 유출 위험 사이에서 사용자가 선택할 수 있는 profile은 무엇인가?

이 질문들은 기능 우선순위 때문에 남긴 항목이 아니라, 실제 사용자·학교 정책·법적 권한·선호가 있어야만 올바르게 확정할 수 있는 end-state의 configuration points다.

---

# 부록 A. 28개 구조적 질문에 대한 명시적 답

1. **단일 source of truth는 무엇인가?** 로컬 Evidence Vault의 immutable artifact, append-only event/claim ledger, canonical entity registry다. 그래프는 projection이다.
2. **네 Graph는 하나인가?** 논리적으로 분리된 bounded context이며 stable ID/typed claim으로 한 좌표계에 결합한다. 민감도와 lifecycle 때문에 물리 저장·키·sync는 분리할 수 있다.
3. **Course, Offering, Lecture, Assessment 경계는?** Course는 지속 정체성, Revision은 유효한 catalog 정의, Offering은 학기 분반, Lecture는 실제 세션, Assessment는 실제 평가다.
4. **Concept와 Competency 차이는?** Concept는 지식 단위, Competency는 조건과 품질 기준이 있는 관찰 가능한 수행이다.
5. **Evidence 저장 단위는?** immutable Artifact의 정확한 page/line/segment/timestamp/snapshot span을 가리키는 EvidenceItem과 이를 쓰는 atomic Claim이다.
6. **Knowledge State와 Freshness 갱신은?** mastery는 수행 evidence로 새 assertion을 만들고 시간만으로 하락하지 않는다. freshness는 최근 strong evidence·반복·종류로 별도 재계산한다.
7. **AI inference와 confirmed fact 구분은?** `status`, creator/model run, source, confidence, scope, UI badge를 분리한다. official fact에는 AI confidence를 쓰지 않는다.
8. **transcript 정보 손실 검증은?** segment/token coverage, 순서, chunk timeline, capture mapping, render QA를 deterministic gate로 검증하며 unmapped가 있으면 incomplete다.
9. **SNU curriculum versioning은?** 입학년도·효력기간·원문 hash·경과조치를 가진 immutable CurriculumVersion과 CourseRevision을 publish한다.
10. **졸업요건 rule version은?** typed executable RequirementSet을 source snapshot에 묶고, profile selector와 proof tree로 평가하며 과거 rule을 수정하지 않는다.
11. **개설예측과 공식 개설 구분은?** `CONFIRMED`, `HISTORICALLY_LIKELY`, `UNCERTAIN`을 type과 UI에서 분리하고 prediction은 졸업 확정 계산에 사용하지 않는다.
12. **강의평 편향·출처는?** Offering/교수/학기/source에 연결하고 원문 provenance, 표본 수·최근성·selection bias·수집 권한을 표시한다.
13. **repository snapshot 저장은?** commit/dirty manifest, blob hash, branch, timestamp, analyzer/policy versions를 가진 immutable aggregate다.
14. **dependency와 실제 사용 구분은?** manifest presence, import, reachable call/config, test scope, runtime evidence를 단계화한다. 설치만으로 observed가 아니다.
15. **spec와 code 충돌 시 무엇이 우선인가?** 현재 구현 질문에는 code/config/runtime, 의도 질문에는 승인된 spec/ADR가 권위다. 충돌은 drift로 보존한다.
16. **세 project 분류 기준은?** OBSERVED는 직접 실행/구조 evidence, REQUIRED는 현재 failure/responsibility proof chain, WOULD_BENEFIT는 미래 trigger와 trade-off가 필요하다.
17. **project evidence가 state에 미치는 영향은?** 사용자의 authorship·이해·결과가 확인될 때만 해당 facet에 반영한다. repo 사용 사실만으로 승격하지 않는다.
18. **Critical Path 비용함수·제약은?** effort, freshness, risk, uncertainty, calendar, switching, opportunity cost의 벡터와 goal/project/curriculum benefit; AND/OR prerequisite, schedule, privacy, user exclusion을 제약으로 쓴다.
19. **여러 path 표현은?** Pareto frontier의 named alternatives와 shared spine/branch, 가정·trade-off를 나란히 보여준다.
20. **Simulator 과대예측 제한은?** future mastery를 쓰지 않고 exposure/practice/assessment opportunity만 projection하며 actual state와 type을 분리한다.
21. **Blind Spot과 관심 없음 구분은?** evidence 부족 분류와 user disposition을 별도 저장한다. `NOT_RELEVANT`는 경고와 추천에서 제외한다.
22. **Question 해결 판단자는?** 사용자다. AI와 자료는 resolution candidate/evidence만 제공한다.
23. **질문 변화로 성장 표시는?** reframe chain에서 scope, prerequisite depth, trade-off, 조건, evidence 사용의 변화를 근거와 함께 서술한다. 단일 난이도 점수는 쓰지 않는다.
24. **수천 concept UX는?** semantic zoom, stable clustering, focus, local neighborhood, lens 제한, path highlight, evidence drill-down을 사용한다.
25. **synonym/alias/granularity는?** ConceptSense와 alias registry, unresolved mention, non-destructive merge/split 및 evidence 재분류 queue로 처리한다.
26. **사용자 수정 우선순위는?** 개인 상태·의도에는 user decision이 최우선이다. 공식 사실 자체는 바꾸지 않고 applicability dispute를 남긴다. AI는 어느 경우에도 자동 덮어쓰지 않는다.
27. **private repo/audio의 외부 AI 경계는?** local default, permission tuple, secret/DLP fail-closed, exact payload preview, purpose/provider/retention별 일회 허가와 egress audit다.
28. **여러 해의 time travel은?** bitemporal claims와 immutable evidence events에서 `as-known-at`/`valid-at` projection을 재생하고 snapshot 간 evidence·ontology·analyzer 변화를 구분한다.

---

# 부록 B. 공식 Source Registry와 확인 정책

| Source | 이 문서에서 확인한 용도 | 재확인 조건 |
|---|---|---|
| [CSE 졸업 이수 규정](https://cse.snu.ac.kr/ko/academics/undergraduate/degree-requirements) | 130학점, GPA 2.0, 학번별 첨부, 외국어진행·공대공통·졸업논문 안내 | 졸업 audit 전, 페이지/첨부 hash 변경 시 |
| [CSE 전공 이수 표준 형태](https://cse.snu.ac.kr/ko/academics/undergraduate/curriculum) | 2026학번 적용, 전공표준, 타과 인정·경과조치 | 매 교과과정 개편, 계획 학기 전 |
| [CSE 필수 교양 과목](https://cse.snu.ac.kr/ko/academics/undergraduate/general-studies-requirements) | 2026학번 교양 49학점과 영역 조건 | 입학년도 확정·공통교육 개편 시 |
| [CSE 학부 교과과정](https://cse.snu.ac.kr/ko/academics/undergraduate/courses) | Course code/title/credits/category catalog | CourseRevision sync 시 |
| [CSE 교과목 변경 내역](https://cse.snu.ac.kr/ko/academics/undergraduate/course-changes) | 폐지·대체·변경 | 매 학기/개편 공지 시 |
| [CSE 학부 안내](https://cse.snu.ac.kr/ko/academics/undergraduate/guide) | 유사과목·다전공 공통 안내 | 인정 판정 전 |
| [2027학번 컴퓨터프로그래밍 전필 제외](https://cse.snu.ac.kr/community/notice/25220) | 미래 curriculum change | 2027 표준형태 게시 시 교차 확인 |
| [공통교육과정 영역 인정 기준](https://cse.snu.ac.kr/community/notice/25379) | 입학년도 기준 영역 인정 원칙 | 실제 교양 판정 전 |
| [2026-2 수강신청 안내](https://cse.snu.ac.kr/community/notice/25337) | 학기 일정·재수강 A0 안내 | 매 학기 새 안내 시 |
| [서울대학교 수강신청 시스템](https://sugang.snu.ac.kr/) | 실제 Offering·시간표·정원·syllabus | Planner 표시 직전과 신청 직전 |
| [SNU 성적등급표](https://www.snu.ac.kr/academics/resources/certificate/grading) | 4.3 scale, S/U, I, 타교 성적 유의 | GradingScheme 변경 감지 시 |
| [정부 대학생 저작권 안내](https://www.korea.kr/news/policyNewsView.do?newsId=148928381) | 교수자 사전 허락 없는 녹음 금지 안내 | 녹음 정책 설계/법령 변경 시 |
| [개인정보위 생성형 AI 안내](https://www.pipc.go.kr/np/cop/bbs/selectBoardArticle.do?bbsId=BS211&mCode=C040020000&nttId=11414) | 외부 AI 개인정보 처리 원칙 | provider/privacy policy 변경 시 |

공식 source snapshot에는 URL만이 아니라 retrieval timestamp, HTTP metadata, raw bytes, hash, parser version을 보존한다. URL이 같아도 내용이 바뀔 수 있기 때문이다.

---

# 부록 C. 최종 수용 불변조건

완성된 시스템은 다음 검사를 통과해야 한다.

- 어떤 GPA·졸업 결과도 계산에 포함된 course attempt와 rule 원문까지 역추적된다.
- 어떤 `CONFIRMED` CourseOffering도 공식 source와 최근 확인일을 가진다.
- 어떤 lecture PDF도 모든 transcript segment의 처리 상태를 설명한다.
- 어떤 Knowledge State도 evidence와 사용자 decision을 열 수 있다.
- stale freshness가 mastery를 자동 강등한 사례가 0건이다.
- 어떤 project `OBSERVED`도 dependency presence만을 근거로 하지 않는다.
- 어떤 `REQUIRED`도 현재 failure/responsibility chain 없이 생성되지 않는다.
- 어떤 `WOULD_BENEFIT_FROM`도 trigger와 trade-off 없이 생성되지 않는다.
- 어떤 simulator projection도 actual Knowledge State table에 쓰이지 않는다.
- 어떤 question도 AI 단독으로 `RESOLVED`가 되지 않는다.
- 어떤 user override도 AI 재실행으로 사라지지 않는다.
- 어떤 raw private repo/lecture payload도 explicit egress grant 없이 장치를 떠나지 않는다.
- 녹음 permission이 `UNKNOWN`인 Offering에서 Record가 활성화되지 않는다.
- graph/search/vector/PDF를 모두 삭제해도 Evidence Vault에서 재생성할 수 있다.
- 전체 export와 독립 restore가 특정 cloud/LLM vendor 없이 가능하다.

---

## 결론

이 OS의 최종 가치는 더 많은 노트를 만들거나 더 그럴듯한 AI 답을 내는 데 있지 않다. 서로 다른 세계의 사실을 같은 척하지 않으면서도—공식 학사 규칙, 실제 강의 원문, 개인의 질문, concept의 prerequisite, 코드의 실행 증거, project의 의도, 수행 competency와 진로 목표를—한 증거 좌표계에서 왕복 가능하게 만드는 데 있다.

사용자가 보는 최종 답은 하나의 점수가 아니라 다음과 같은 설명 가능한 지도다.

```text
나는 지금 어디에 있는가
  = 현재 Knowledge State + Freshness + 증거와 반례

무엇을 실제로 해봤는가
  = 강의/과제/코드/실험/debugging/설계의 구체적 provenance

무엇이 비어 있는가
  = 목표를 막는 Gap과 판단할 exposure가 없는 Blind Spot의 구분

다음 목표까지 어떤 길이 있는가
  = 하나의 정답이 아닌 Critical Path의 shared spine과 대안

학교와 project는 어떻게 연결되는가
  = versioned SNU Course/Offering과 snapshot 기반 project requirement의 연결

무엇을 결정해야 하는가
  = AI가 숨기지 않은 근거·불확실성·trade-off 위에서 사용자가 내리는 선택
```

이 구조가 유지될 때 시스템은 공부를 대신하지 않으면서도, 수년에 걸친 서울대학교 컴퓨터공학 학업과 개발 경험을 잃지 않고 연결하는 진정한 Personal Academic · CS · Project OS가 된다.
