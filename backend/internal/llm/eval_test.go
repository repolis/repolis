package llm

// Live model evaluation. Skipped unless LLM_EVAL names the profiles to run:
//
//	LLM_EVAL=local,remote go test ./internal/llm -run TestEval -v -count=1 -timeout 30m
//
// LLM_EVAL_PARTS=adjudicate,label,explain picks sections; empty runs all.
//
// "local" uses backend/.env as is. "remote" points the same client at Google
// AI Studio's OpenAI-compatible endpoint with GOOGLE_AI_STUDIO_API_KEY. Any
// profile's models can be overridden with LLM_EVAL_<PROFILE>_FAST and
// LLM_EVAL_<PROFILE>_MODEL, so one run can compare several.
//
// The adjudication cases are real residual associations (rules could not
// settle them) from tsoding/nothing c20e7c4, rgamble/libcsv b1d5212 and
// libgit2/libgit2 0551dfd, hand-labelled. Several are deliberately ones the
// rule gets wrong; `rule` records what the rule chose, for the baseline.

import (
	"context"
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"testing"
	"time"

	"github.com/joho/godotenv"
	"github.com/repolis/repolis/backend/internal/models"
)

type assocCase struct {
	fn, sig string
	options []string
	accept  []string // any of these is correct
	rule    string   // the rule's tentative choice
}

func opts(s string) []string { return strings.Split(s, "|") }

var assocCases = []assocCase{
	// tsoding/nothing - every residual case.
	{"labels_layer_texts", "(const LabelLayer *label_layer)", opts("LabelLayer|Labels|LabelUndoContext|LabelUndoType"), opts("LabelLayer"), "Labels"},
	{"main", "(int argc, char *argv[])", opts("Game|Lt"), opts("NONE"), "Game"},
	{"rect_side_distance", "(Rect rect, Vec2f point, Rect_side side)", opts("Rect|Rect_side|Vec2f|Line"), opts("Rect"), "Rect_side"},
	{"rect_side", "(Rect rect, Rect_side side)", opts("Rect|Rect_side|Line"), opts("Rect|Rect_side"), "Rect_side"},
	{"sprite_font_boundary_box", "(Vec2f position, Vec2f size, const char *text)", opts("Sprite_font|Vec2f|Rect"), opts("Sprite_font|Rect"), "Sprite_font"},
	{"rect_from_point", "(Vec2f p, float w, float h)", opts("Vec2f|Rect"), opts("Rect"), "Rect"},
	{"rect_from_points", "(Vec2f p1, Vec2f p2)", opts("Vec2f|Rect"), opts("Rect"), "Rect"},
	{"rect_from_vecs", "(Vec2f position, Vec2f size)", opts("Vec2f|Rect"), opts("Rect"), "Rect"},
	{"triangle", "(Vec2f p1, Vec2f p2, Vec2f p3)", opts("Vec2f|Triangle"), opts("Triangle"), "Triangle"},
	{"fill_triangle", "(SDL_Renderer *render, Triangle t)", opts("Triangle|Vec2f"), opts("Triangle"), "Vec2f"},
	{"string_to_cstr", "(Memory *memory, String s)", opts("Memory|String"), opts("String"), "String"},

	// rgamble/libcsv - both residual cases.
	{"test_parser", "(char *test_name, unsigned char options, void *input, size_t len, struct event expected[], char delimiter, char quote, int (*space_func)(unsigned char), int (*term_func)(unsigned char))", opts("event|csv_parser"), opts("csv_parser|NONE"), "csv_parser"},
	{"main", "(int argc, char *argv[])", opts("counts|csv_parser"), opts("NONE"), "csv_parser"},

	// libgit2 - library code.
	{"checkout_get_actions", "( uint32_t **actions_ptr, size_t **counts_ptr, checkout_data *data, git_iterator *workdir)", opts("checkout_data|checkout_conflictdata|checkout_stream|git_iterator|git_diff_delta|git_index_entry"), opts("checkout_data"), "git_diff_delta"},
	{"git_repository_index__weakptr", "(git_index **out, git_repository *repo)", opts("git_repository|git_index|mergehead_data|repo_paths|validate_ownership_data|git_index_options"), opts("git_repository"), "git_repository"},
	{"git_tree__write_index", "( git_oid *oid, git_index *index, git_repository *repo)", opts("git_oid|git_tree|git_index|git_repository|tree_key_search|tree_stack_entry"), opts("git_tree"), "git_tree"},
	{"config_file_read_buffer", "( git_config_list *config_list, const git_repository *repo, config_file *file, git_config_level_t level, int depth, const char *buf, size_t buflen)", opts("config_file|git_config_list|git_repository|config_file_backend|config_file_parse_data|write_data"), opts("config_file"), "config_file"},
	{"filesystem_iterator_advance_into", "( const git_index_entry **out, git_iterator *i)", opts("filesystem_iterator|git_index_entry|git_iterator|empty_iterator|filesystem_iterator_entry|filesystem_iterator_frame"), opts("filesystem_iterator"), "filesystem_iterator"},
	{"git_remote_set_autotag", "(git_repository *repo, const char *remote, git_remote_autotag_option_t value)", opts("git_repository|git_remote|git_remote_autotag_option_t|refspec_cb_data|update_data|git_config"), opts("git_remote"), "git_remote"},
	{"git_repository_item_path", "( git_buf *out, const git_repository *repo, git_repository_item_t item)", opts("git_repository|git_buf|git_repository_item_t|mergehead_data|repo_paths|validate_ownership_data"), opts("git_repository"), "git_repository"},
	{"git_remote_list", "(git_strarray *remotes_list, git_repository *repo)", opts("git_strarray|git_remote|git_repository|refspec_cb_data|update_data|git_config"), opts("git_remote"), "git_remote"},
	{"git_index_conflict_next", "( const git_index_entry **ancestor_out, const git_index_entry **our_out, const git_index_entry **their_out, git_index_conflict_iterator *iterator)", opts("git_index_entry|git_index|entry_common|entry_internal|entry_srch_key|entry_time"), opts("git_index"), "git_index"},
	{"index_iterator_reset", "(git_iterator *i)", opts("index_iterator|git_iterator|empty_iterator|filesystem_iterator|filesystem_iterator_entry|filesystem_iterator_frame"), opts("index_iterator"), "index_iterator"},
	{"git_submodule__resolve_url", "( git_str *out, git_repository *repo, const char *url)", opts("git_submodule|git_str|git_repository|fbp_data|lfc_data"), opts("git_submodule"), "git_submodule"},
	{"refdb_fs_backend__init", "(struct git_refdb_backend *_backend, const char *head_target, mode_t mode, uint32_t flags)", opts("refdb_fs_backend|git_refdb_backend|iter_load_context|packref|refdb_fs_iter|git_str"), opts("refdb_fs_backend"), "refdb_fs_backend"},
	{"git_object__is_valid", "( git_repository *repo, const git_oid *id, git_object_t expected_type)", opts("git_repository|git_object|git_oid|git_object_t|git_object_def|git_odb"), opts("git_object"), "git_object"},
	{"git_remote_set_url", "(git_repository *repo, const char *remote, const char *url)", opts("git_repository|git_remote|refspec_cb_data|update_data"), opts("git_remote"), "git_remote"},
	{"git_commit_create_buffer", "( git_buf *out, git_repository *repo, const git_signature *author, const git_signature *committer, const char *message_encoding, const char *message, const git_tree *tree, size_t parent_count, const git_commit *parents[])", opts("git_buf|git_commit|git_signature|git_repository|commit_parent_data|commit_parent_oids"), opts("git_commit"), "git_commit"},
	{"git_diff_print_callback__to_file_handle", "( const git_diff_delta *delta, const git_diff_hunk *hunk, const git_diff_line *line, void *payload)", opts("git_diff_delta|git_diff|git_diff_hunk|git_diff_line|diff_print_info"), opts("git_diff"), "git_diff"},
	{"git_submodule_open", "(git_repository **subrepo, git_submodule *sm)", opts("git_submodule|git_repository|fbp_data|lfc_data"), opts("git_submodule"), "git_submodule"},
	{"git_commit_list_parse", "(git_revwalk *walk, git_commit_list_node *commit)", opts("git_commit_list|git_revwalk|git_commit_list_node|git_commit_graph_entry|git_commit_graph_file|git_odb_object"), opts("git_commit_list|git_commit_list_node"), "git_commit_list"},
	{"index_entry_dup_nocache", "( git_index_entry **out, git_index *index, const git_index_entry *src)", opts("git_index_entry|index_entry|git_index|entry_common|entry_internal|entry_srch_key"), opts("index_entry|git_index_entry"), "index_entry"},
	{"tree_iterator_advance_into", "( const git_index_entry **out, git_iterator *i)", opts("tree_iterator|git_index_entry|git_iterator|empty_iterator|filesystem_iterator|filesystem_iterator_entry"), opts("tree_iterator"), "tree_iterator"},
	{"git_remote_rename", "(git_strarray *out, git_repository *repo, const char *name, const char *new_name)", opts("git_strarray|git_remote|git_repository|refspec_cb_data|update_data|git_vector"), opts("git_remote"), "git_remote"},
	{"git_blob_create_from_buffer", "( git_oid *id, git_repository *repo, const void *buffer, size_t len)", opts("git_oid|git_blob|git_repository|blob_writestream|git_odb_stream|git_odb"), opts("git_blob"), "git_blob"},
	{"git_packbuilder_write_buf", "(git_buf *buf, git_packbuilder *pb)", opts("git_packbuilder|git_buf|pack_write_context|thread_params|tree_walk_context|unpacked"), opts("git_packbuilder"), "git_packbuilder"},
	{"git_http_client_read_response", "( git_http_response *response, git_http_client *client)", opts("git_http_client|git_http_response|git_http_server|git_http_server_t|http_client_state|http_parser_context"), opts("git_http_client"), "git_http_client"},
	{"config_file_write", "( config_file_backend *cfg, const char *orig_key, const char *key, const git_regexp *preg, const char *value)", opts("config_file|config_file_backend|config_file_parse_data|write_data|git_config_parser|git_filebuf"), opts("config_file|config_file_backend"), "config_file"},
	{"git_tag_create_from_buffer", "(git_oid *oid, git_repository *repo, const char *buffer, int allow_ref_overwrite)", opts("git_oid|git_tag|git_repository|tag_cb_data|tag_filter_data|git_odb_stream"), opts("git_tag"), "git_tag"},
	{"git_repository_head_for_worktree", "(git_reference **out, git_repository *repo, const char *name)", opts("git_repository|git_reference|mergehead_data|repo_paths|validate_ownership_data|git_worktree"), opts("git_repository"), "git_repository"},
	{"git_ignore_path_is_ignored", "( int *ignored, git_repository *repo, const char *pathname)", opts("git_repository|git_attr_file|git_attr_path|git_dir_flag|git_ignores"), opts("git_ignores|git_repository"), "git_attr_file"},
	{"cmd_blame", "(int argc, char **argv)", opts("git_blame_options|cli_repository_open_options|cli_opt|git_blame|git_repository|git_str"), opts("git_blame|NONE"), "git_blame_options"},

	// libgit2 - tests, where the suite name says what is under test.
	{"test_revwalk_simplify__first_parent", "(void)", opts("git_oid|git_repository|git_revwalk"), opts("git_revwalk"), "git_oid"},
	{"test_index_tests256__remove_directory", "(void)", opts("test_entry|git_index|git_repository"), opts("git_index"), "git_index"},
	{"test_diff_workdir__to_index_pathlist", "(void)", opts("git_diff_options|git_diff|git_index|git_vector"), opts("git_diff"), "git_diff_options"},
	{"test_network_remote_local__push_to_non_bare_remote", "(void)", opts("git_fetch_options|git_strarray|git_remote|git_repository"), opts("git_remote"), "git_fetch_options"},
	{"test_iterator_index__pathlist_2", "(void)", opts("git_index|git_iterator|git_iterator_options|git_vector"), opts("git_iterator|git_index"), "git_index"},
}

type labelCase struct {
	symbols, dirs []string
	size          int
	noise         []string
	expect        []string // a correct name contains one of these (lower case)
}

var labelCases = []labelCase{
	{[]string{"rect_from_points", "rect_contains_point", "vec_sum", "vec_length", "triangle_contains", "line_intersect", "mat3x3_product"}, []string{"src/math"}, 41, nil, []string{"geometr", "math", "vector", "linear"}},
	{[]string{"sprite_font_load", "sprite_font_render_text", "renderer_fill_rect", "camera_render_text", "texture_from_bmp"}, []string{"src/sdl", "src/game"}, 23, nil, []string{"render", "font", "text", "graphic", "draw", "sprite"}},
	{[]string{"odb_read", "odb_write", "loose_backend__read", "pack_backend__exists", "odb_hash", "odb_foreach"}, []string{"src/libgit2/odb", "src/libgit2/pack"}, 64, []string{"git", "libgit2"}, []string{"object", "storage", "odb", "pack", "database"}},
	{[]string{"http_client_read_response", "http_client_send_request", "http_parser_on_header", "transport_smart_connect", "ssh_stream_read"}, []string{"src/libgit2/transports"}, 52, []string{"git", "libgit2"}, []string{"transport", "http", "network", "protocol", "connection"}},
	{[]string{"config_file_read", "config_file_write", "config_parse", "config_get_string", "config_set_int32", "config_backend_snapshot"}, []string{"src/libgit2/config"}, 47, []string{"git", "libgit2"}, []string{"config"}},
	{[]string{"diff_print_patch", "patch_generate", "diff_find_similar", "diff_tform_rename", "xdl_diff", "diff_stats_to_buf"}, []string{"src/libgit2/diff", "deps/xdiff"}, 70, []string{"git", "libgit2"}, []string{"diff", "patch", "comparison"}},
	{[]string{"sha1_update", "sha256_final", "hash_ctx_init", "hash_buf", "oid_fmt", "oid_cmp"}, []string{"src/util/hash"}, 18, []string{"git", "libgit2"}, []string{"hash", "sha", "digest", "checksum", "oid"}},
	{[]string{"csv_parse", "csv_fini", "csv_set_delim", "csv_write", "csv_fwrite", "csv_init"}, []string{"root"}, 12, nil, []string{"csv", "pars", "delimit", "writ"}},
}

type explainCase struct {
	b      models.Building
	source string
	expect []string
}

var explainCases = []explainCase{
	{models.Building{Name: "Rect", Kind: "struct", SourceFile: "src/math/rect.h", Language: "c", Fields: []string{"x", "y", "w", "h"}, NumFields: 4,
		Methods: []string{"rect_from_points", "rect_contains_point", "rect_side_distance", "rects_overlap"}, NumMethods: 4},
		"typedef struct {\n    float x, y, w, h;\n} Rect;", []string{"rectangle"}},
	{models.Building{Name: "csv_parser", Kind: "struct", SourceFile: "libcsv.c", Language: "c",
		Fields: []string{"pstate", "quoted", "spaces", "entry_buf", "entry_pos", "entry_size", "status", "options", "quote_char", "delim_char"}, NumFields: 10,
		Methods: []string{"csv_init", "csv_parse", "csv_fini", "csv_set_delim", "csv_free"}, NumMethods: 5},
		"struct csv_parser {\n  int pstate;\n  int quoted;\n  size_t spaces;\n  unsigned char * entry_buf;\n  size_t entry_pos;\n  size_t entry_size;\n  int status;\n  unsigned char options;\n  unsigned char quote_char;\n  unsigned char delim_char;\n};",
		[]string{"csv"}},
	{models.Building{Name: "git_revwalk", Kind: "struct", SourceFile: "src/libgit2/revwalk.h", Language: "c",
		Fields: []string{"odb", "commits", "iterator_topo", "iterator_rand", "iterator_reverse", "sorting", "hide_cb"}, NumFields: 7,
		Methods: []string{"git_revwalk_new", "git_revwalk_push", "git_revwalk_hide", "git_revwalk_next", "git_revwalk_sorting"}, NumMethods: 5},
		"", []string{"commit", "histor", "walk"}},
}

type evalProfile struct {
	name, baseURL, apiKey, fast, rich, effort string
	concurrency                               int
	// Requests per minute, 0 for unlimited. AI Studio's free tier allows 15
	// per model; latency is measured after the wait, throughput is not.
	rpm int
}

var paceMu sync.Mutex
var paceNext time.Time
var paceGap time.Duration

// pace blocks until the profile's request rate allows another call.
func pace() {
	if paceGap == 0 {
		return
	}
	paceMu.Lock()
	now := time.Now()
	if paceNext.Before(now) {
		paceNext = now
	}
	wait := paceNext.Sub(now)
	paceNext = paceNext.Add(paceGap)
	paceMu.Unlock()
	time.Sleep(wait)
}

func loadProfiles(t *testing.T) []evalProfile {
	env, err := godotenv.Read(filepath.Join("..", "..", ".env"))
	if err != nil {
		t.Fatalf("reading backend/.env: %v", err)
	}
	get := func(k, def string) string {
		if v := os.Getenv(k); v != "" {
			return v
		}
		if v, ok := env[k]; ok && v != "" {
			return v
		}
		return def
	}

	var out []evalProfile
	for _, name := range strings.Split(os.Getenv("LLM_EVAL"), ",") {
		name = strings.TrimSpace(name)
		var p evalProfile
		switch name {
		case "local":
			p = evalProfile{name: name, baseURL: get("LLM_BASE_URL", ""), apiKey: get("LLM_API_KEY", ""),
				fast: get("LLM_MODEL_FAST", ""), rich: get("LLM_MODEL", ""), effort: get("LLM_REASONING_EFFORT", "none"), concurrency: 2}
		case "remote":
			p = evalProfile{name: name, baseURL: "https://generativelanguage.googleapis.com/v1beta/openai/",
				apiKey: get("GOOGLE_AI_STUDIO_API_KEY", ""), fast: "gemini-3.5-flash-lite", rich: "gemini-3.5-flash-lite",
				effort: "minimal", concurrency: 8, rpm: 14}
			if p.apiKey == "" {
				t.Fatal("remote profile needs GOOGLE_AI_STUDIO_API_KEY")
			}
		default:
			t.Fatalf("unknown LLM_EVAL profile %q", name)
		}
		up := strings.ToUpper(name)
		p.fast = get("LLM_EVAL_"+up+"_FAST", p.fast)
		p.rich = get("LLM_EVAL_"+up+"_MODEL", p.rich)
		p.effort = get("LLM_EVAL_"+up+"_EFFORT", p.effort)
		if v := get("LLM_EVAL_"+up+"_RPM", ""); v != "" {
			fmt.Sscan(v, &p.rpm)
		}
		out = append(out, p)
	}
	return out
}

func (p evalProfile) client(t *testing.T) *Client {
	t.Setenv("LLM_BASE_URL", p.baseURL)
	t.Setenv("LLM_API_KEY", p.apiKey)
	t.Setenv("LLM_MODEL_FAST", p.fast)
	t.Setenv("LLM_MODEL", p.rich)
	t.Setenv("LLM_REASONING_EFFORT", p.effort)
	t.Setenv("LLM_CONCURRENCY", fmt.Sprint(p.concurrency))
	c, err := NewClient(nil) // no cache: every call is measured
	if err != nil {
		t.Fatal(err)
	}
	return c
}

type latencies struct {
	mu sync.Mutex
	d  []time.Duration
}

func (l *latencies) add(d time.Duration) { l.mu.Lock(); l.d = append(l.d, d); l.mu.Unlock() }

func (l *latencies) pct(p float64) time.Duration {
	if len(l.d) == 0 {
		return 0
	}
	s := append([]time.Duration(nil), l.d...)
	sort.Slice(s, func(i, j int) bool { return s[i] < s[j] })
	return s[int(p*float64(len(s)-1)+0.5)]
}

func (l *latencies) String() string {
	return fmt.Sprintf("p50 %4dms  p95 %4dms", l.pct(0.5).Milliseconds(), l.pct(0.95).Milliseconds())
}

func contains(list []string, s string) bool {
	for _, v := range list {
		if v == s {
			return true
		}
	}
	return false
}

func TestEval(t *testing.T) {
	if os.Getenv("LLM_EVAL") == "" {
		t.Skip("set LLM_EVAL=local,remote to run live model evaluation")
	}
	for _, p := range loadProfiles(t) {
		t.Run(p.name, func(t *testing.T) {
			c := p.client(t)
			paceGap = 0
			if p.rpm > 0 {
				paceGap = time.Minute / time.Duration(p.rpm)
			}
			ctx := context.Background()
			t.Logf("profile %s: fast=%s rich=%s effort=%q concurrency=%d rpm=%d", p.name, p.fast, p.rich, p.effort, p.concurrency, p.rpm)
			if p.rpm == 0 {
				// Load the models first so the first case does not pay for it.
				_, _ = c.complete(ctx, c.adjudicateRequest("c", "f", "()", []string{"A", "B"}))
				if p.rich != p.fast {
					_, _ = c.complete(ctx, request{kind: "warm", model: p.rich, user: "Say OK.", maxTokens: 4})
				}
			}
			parts := os.Getenv("LLM_EVAL_PARTS") // e.g. "explain"; empty runs all
			run := func(name string) bool { return parts == "" || strings.Contains(parts, name) }
			if run("adjudicate") {
				evalAdjudicate(t, c)
			}
			if run("label") {
				evalLabels(t, c)
			}
			if run("explain") {
				evalExplain(t, c)
			}
		})
	}
}

func evalAdjudicate(t *testing.T, c *Client) {
	ctx := context.Background()
	var lat latencies
	type result struct {
		raw, ans string
		err      error
	}
	start := time.Now()
	results := runBatch(ctx, c.concurrency, len(assocCases), func(i int) result {
		k := assocCases[i]
		pace()
		t0 := time.Now()
		out, err := c.complete(ctx, c.adjudicateRequest("c", k.fn, k.sig, k.options))
		lat.add(time.Since(t0))
		return result{raw: out, ans: cleanAnswer(out), err: err}
	})
	wall := time.Since(start)

	var modelOK, ruleOK, appliedOK, empty, invalid, ruleWrong, fixed, broke int
	for i, r := range results {
		k := assocCases[i]
		rOK := contains(k.accept, k.rule)
		valid := r.err == nil && (r.ans == "NONE" || contains(k.options, r.ans))
		mOK := valid && contains(k.accept, r.ans)
		// What AdjudicateAssociations would leave: the model's valid answer,
		// otherwise the rule's tentative one.
		final := k.rule
		if valid {
			final = r.ans
		}
		aOK := contains(k.accept, final)

		switch {
		case r.err != nil || r.ans == "":
			empty++
		case !valid:
			invalid++
		}
		if mOK {
			modelOK++
		}
		if rOK {
			ruleOK++
		} else {
			ruleWrong++
			if aOK {
				fixed++
			}
		}
		if rOK && !aOK {
			broke++
		}
		if aOK {
			appliedOK++
		}
		if !mOK {
			t.Logf("  assoc miss: %-42s got %-24q want %v (rule %s)", k.fn, r.raw, k.accept, k.rule)
		}
	}
	n := len(assocCases)
	t.Logf("ADJUDICATE n=%d  model %d/%d (%.0f%%)  rule-only %d/%d (%.0f%%)  pipeline %d/%d (%.0f%%)",
		n, modelOK, n, pct(modelOK, n), ruleOK, n, pct(ruleOK, n), appliedOK, n, pct(appliedOK, n))
	t.Logf("ADJUDICATE rule-wrong cases fixed %d/%d, rule-right cases broken %d  empty %d  invalid %d",
		fixed, ruleWrong, broke, empty, invalid)
	t.Logf("ADJUDICATE latency %s  wall %dms (%.1f calls/s, includes rate-limit waits)", lat.String(), wall.Milliseconds(), float64(n)/wall.Seconds())
}

func evalLabels(t *testing.T, c *Client) {
	ctx := context.Background()
	var lat latencies
	var parsed, specific, onTopic, echo int
	start := time.Now()
	outs := runBatch(ctx, c.concurrency, len(labelCases), func(i int) string {
		k := labelCases[i]
		pace()
		t0 := time.Now()
		out, err := c.complete(ctx, c.labelRequest("c", k.symbols, k.dirs, k.size, k.noise))
		lat.add(time.Since(t0))
		if err != nil {
			return ""
		}
		return out
	})
	wall := time.Since(start)
	for i, out := range outs {
		k := labelCases[i]
		l := parseLabel(stripThinking(out))
		if l == nil || l.Name == "" {
			t.Logf("  label %d: unparsed %q", i+1, out)
			continue
		}
		parsed++
		if strings.EqualFold(l.Name, exampleName) {
			echo++
			continue
		}
		name := stripProjectToken(l.Name, k.noise)
		if !isVague(name, k.noise) {
			specific++
		}
		low := strings.ToLower(name + " " + l.Summary)
		hit := false
		for _, e := range k.expect {
			if strings.Contains(low, e) {
				hit = true
				break
			}
		}
		if hit {
			onTopic++
		}
		t.Logf("  label %d: %-28q %-50q on-topic=%v", i+1, name, l.Summary, hit)
	}
	n := len(labelCases)
	t.Logf("LABEL n=%d  parsed %d  specific %d  on-topic %d/%d (%.0f%%)  echoed example %d",
		n, parsed, specific, onTopic, n, pct(onTopic, n), echo)
	t.Logf("LABEL latency %s  wall %dms", lat.String(), wall.Milliseconds())
}

func evalExplain(t *testing.T, c *Client) {
	ctx := context.Background()
	var lat latencies
	var ok, onTopic, markdown int
	for i, k := range explainCases {
		pace()
		t0 := time.Now()
		out, err := c.ExplainBuilding(ctx, k.b, k.source, nil, nil)
		lat.add(time.Since(t0))
		if err != nil || out == "" {
			t.Logf("  explain %d (%s): failed: %v", i+1, k.b.Name, err)
			continue
		}
		ok++
		low := strings.ToLower(out)
		for _, e := range k.expect {
			if strings.Contains(low, e) {
				onTopic++
				break
			}
		}
		if strings.ContainsAny(out, "*#`") || strings.Contains(out, "\n- ") {
			markdown++
		}
		t.Logf("  explain %d (%s, %dms): %s", i+1, k.b.Name, lat.d[len(lat.d)-1].Milliseconds(), out)
	}
	n := len(explainCases)
	t.Logf("EXPLAIN n=%d  answered %d  on-topic %d  markdown %d  latency %s", n, ok, onTopic, markdown, lat.String())
}

func pct(a, b int) float64 {
	if b == 0 {
		return 0
	}
	return 100 * float64(a) / float64(b)
}
