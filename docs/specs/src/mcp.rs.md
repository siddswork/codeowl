---
kind: file
source_paths: [src/mcp.rs]
file: { source_hash: 88e3218a8211427c7f15c2f89351fb53063991a5ca91befc6aae21332a6f10d1, deps_hash: 9444e52185555f611607b32518e97fa859338da2fec05d5fc9f83b1823fcf766, spec_hash: 980c9cf0ea48a13476da1c34104a2fe426ad9ddc024789af3c50465b4fd3b070 }
symbols:
  src/mcp.rs::IdRequest: { source_hash: 761e58dcd159609a23d132efeeb75de5ee10d926ebdddf739a436bf5a6ece393, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: fdf89e878c4c372ef27ba372a491362b0934c3fbb4b26af24a7328975599932a }
  src/mcp.rs::SourceRequest: { source_hash: b7aab31266401c38a91e75b3b2261a31c8de9bca2b7232b4d661cb4d0439ead1, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 44285528c4e4802813ecfe2f78f452544b01ffef2da1c152a7126af12e7d24e9 }
  src/mcp.rs::SourceResponse: { source_hash: b67fd20637febae4f449a62df5993ad3bd1a11edcc0cb3357b1e9f67e0a7420e, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: f915880b9e5207ac578ddf55734ccb0816eb7d468fffb806ca298ce2dca65668 }
  src/mcp.rs::SearchRequest: { source_hash: 1046c1854a7426865fee5d9e620a1d47d37d6d515547aab4468d3bfb085344ea, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 3a08a946510b24b4c6aaed361bf930a52854909b734756e3a2c284946532a776 }
  src/mcp.rs::CoverageRequest: { source_hash: 27f49d46f7bc9edaa003e6450b4029ba82dccbfeca933825e7792ff0032e740e, deps_hash: 73c62cd767ffad9259981bffd9f8f9ae05d94a4a41e6bbf97b734e521f1e65f4, spec_hash: 7440009a1abab991f7914e197b42f3cc37fa6d6752d3b89d9d2d7a324ce50c49 }
  src/mcp.rs::OrphanedSpecResponse: { source_hash: 9f63db4102b6e8d4a9d1465ec7f2b068bc4b2adf74b926210db2455f55c2927d, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 171f1a4c1a3320a14b1d8c81f0f9de58d47acded3664969c83294a4b074cd229 }
  src/mcp.rs::impl From<crate::spec::OrphanedSpec> for OrphanedSpecResponse: { source_hash: 39aee3310b0ceace62072374a1220bc3f0e434820a72c66ba16c0d8c00e57a48, deps_hash: 2fe130e5c207310c2cda2aab1046230057f7df14901b3333ebff4526a249deb8, spec_hash: 01d5f50b5352403650cf2080e05c91b7e656fd750b08be8443b1e86617db5bd2 }
  src/mcp.rs::impl From<crate::spec::CoverageItem> for CoverageItemResponse: { source_hash: 8944078a6b1d001ff16658314912350474760108716c15977e8b91e460aa2d8e, deps_hash: fb8a7d483b720df3b665ad73c2f35b240cdd6c0938e566d84182b8d9da4c4a22, spec_hash: fec9073e460a98872aea257198cd8d19985b6b191f79b92aa27cd20f3716769c }
  src/mcp.rs::CoverageItemResponse: { source_hash: 3c7f0288a9ef9815dce69b2444211c149154e19c9ddf5b5762266c616ee5d10a, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 7890b5c5e2ef2446647ec57479ed4efb4c0a05c7c2c3fc9e33215168a4688c56 }
  src/mcp.rs::KindBreakdown: { source_hash: 4300f68f6bc2df18768d1ad7a28967dd29ebcf3e0cb25ec2edaf75234c60d57a, deps_hash: 955f5bde621a139600bd68be865eeda697dc4e55986cea522e79d5e371bda457, spec_hash: 55fb0bc8a6a7b0b93d3123a501d2642555362f59c3e47cac6729e187f3d7b1ec }
  src/mcp.rs::ModuleBreakdown: { source_hash: ae9e4f6f182aa9a73020fc7a32e85b6d39be317bca4bdcc17000ceaadad180a2, deps_hash: ad1ffde702f108cabf2fd86c0bc9f124104fb13ab6eac43753512113d67f8f52, spec_hash: 82fbc0283de79b7cad636c8d5847556d1dfb19bab4cf5b7bb6e0a4a260c33a9e }
  src/mcp.rs::impl From<(String, crate::spec::CoverageSummary)> for KindBreakdown: { source_hash: d0ebd238186846a55970b349281361e6a3a51455365223aee5ed45b612e439ba, deps_hash: 7c1688b1c363fda6c60c3d4e3f3867c8b7e14525a0e838de0ed4738d58ef46d1, spec_hash: f5ebd15f8b620804afbcd48874330940cde675daa44dd2996898af750767b6dd }
  src/mcp.rs::impl From<(String, crate::spec::CoverageSummary)> for ModuleBreakdown: { source_hash: 48c40c8dbff109998332934b37984a442015e53968fa67827fb3898a755a052f, deps_hash: 732583bc9f4a1d35d5d8e02fdc8074b8f53978cfeaa33dd5e0a0a0e8e2406126, spec_hash: 0dc3aaae9ecec824eb59a334cf150ceb30629da7c3f50722faadf544810bc140 }
  src/mcp.rs::CoverageResponse: { source_hash: 94eefe0fa640b87316ec8f683ac4e98f478ba21e64d44ddcbba047682dca95f3, deps_hash: df58b852dfa93e337ddc1e3b5a1ba35dcf68cb9fe9b5728ab16a7e6101c7fdb1, spec_hash: 05fde9d46b1dfea632fed47593f86355bb2215a9673db895579905ddb99a635c }
  src/mcp.rs::GeneratedSourcesResponse: { source_hash: 3952b15546156d4887f0b6db9fd6b6b49bb3a545056f1f0f13366648619727b7, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 3968b663675edd5aa565ae49945cfbf583df63e1190aec857bf01668cede453a }
  src/mcp.rs::impl From<crate::spec::GeneratedSourcesSummary> for GeneratedSourcesResponse: { source_hash: edbd2a6ec14a443251868cc7ab20571974f776eb29e5f51fdca6d15ddd818d0a, deps_hash: 2fe2969c0812cb6115a2c5961d9a1a0850cff9cbe595d85452ffcfbc11d82544, spec_hash: 5de4aee30fe5ac7c1efbb2e8a6ef106c5d6c937a4065b24aac22af8301791ad4 }
  src/mcp.rs::CallerInfo: { source_hash: a6812275a3ca04e2ede9048a3f7a82b3b3fa8ff7849c1fe9641e9d885b97e333, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: b20445a9172c162482f07f32ad5cf51fb54d7130a7936a99717aec1d5c828244 }
  src/mcp.rs::CallersResponse: { source_hash: e044bdb05db167965d86b2d6c0fcb6135e418af27cd748f7d40bfa7bcf4a2dd8, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: a33e0c490d6fbeddf253e87c0da2df4e2ef024f346f0f4cc60d71fa91663821d }
  src/mcp.rs::CalleeInfo: { source_hash: 25aa422e13b1e1c05a3bc582aaeefca5008c81f764b8ebeeff19a339f161ec8a, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: dd5671280e92a324e22f65ea6a487d79ee0a99d59a73197ab8cebdf497481beb }
  src/mcp.rs::CalleesResponse: { source_hash: 08664613bcfb71a724374004d95e7b8b892e70c79fd7873d6fc0a455a85e2edc, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 9ce6b08a618b62eebc6c2a89fcf6ea2c351863ad95ac098e6f0f3b6ed9c52676 }
  src/mcp.rs::SearchResponse: { source_hash: e78bfedbed9b06d65356c6e50c5c1272810a6a64406b06838c06fb19a07a143f, deps_hash: 46a8bf5520fa73ee675851a0967a5921e01f32cc688d9fe8ef5be79aeac104d9, spec_hash: d8428abc083faa8188e2b1236c080d37bff3dd49df994b0052cff8f8e2258356 }
  src/mcp.rs::SpecResponse: { source_hash: c0d28b274849fb85a43cb68dc22f40598d767cac314cc967dae7b6760ea4776e, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 152da740f41bac5a35d2f6829e01c8340bbbfd50daa953de828daf624c8d3e0c }
  src/mcp.rs::GenerateTaskRequest: { source_hash: c093bd5a566d7bc001961f49c33b128fd84b72935c5ff6341c062be761a02bd6, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: b02105ffd29547f652d12e9e5ee9aa5a565706f18033b0fec6e0b3a047ed93aa }
  src/mcp.rs::CoreSource: { source_hash: dd9afc4f10c29843e8922b95fdf28263852fbb75e77a70120cf5a7fd18b5ff08, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 47919d9ea0186e71e641b5765d8689e80d5b8eb86ed332e41f68563f1b096a02 }
  src/mcp.rs::DependencyContext: { source_hash: 9edbbf2c7c5d840f47fe642fd1faf063f2f71ef6edf929daa7b23e18ee082b08, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 2a7b26ff543ad3391eef4d5f6fe1945bd446bb8d8f3cc06a557f5b14b4b74179 }
  src/mcp.rs::TableContext: { source_hash: 22cd52f1cc85d160857974475fc18988e985540b77578fa983be7d7e19e97fcf, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: cf3de1eb9284af4345bc4a6505cb94a1fd674883a92ddb5c10ccdb4f97906230 }
  src/mcp.rs::RollupFile: { source_hash: 15218353b03ce7a5bbd72a14f05d818d8ffd036b2b564eacdcf979c933500548, deps_hash: 8a332bbfaa5813172d43d38f49781291ef92d2cae250b92807fbdeec1cfb48a2, spec_hash: 0b6f37fd031c731d7add7610fed059d32113b84a90948f951b2248c46c4a32a8 }
  src/mcp.rs::SpecTaskResponse: { source_hash: cd1e68d7c9c2e3b25f099909f664b7e86cc6d2af12aae614948685eaa8bf8c4f, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 531f3301637dd556e372926144982046508731d7dc72b41b8d62f56d382398e3 }
  src/mcp.rs::ModuleSummary: { source_hash: 69d0c150e03638a03de3aaaf615dac65069b57009ddbf8188ff4748d3cee2230, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: ffdf97054d461bbc96b2cad8e8e13f19930090b1cd9538a27bf8fb1f53198f2d }
  src/mcp.rs::FeatureSummary: { source_hash: b3538455a3db231839be9f3a6c4708dbff1074fb3f79fca456509cebb36a9ca1, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: a6cfa576b517915a87277a36936fade09bf8be7eebad2c994224bb84e6489235 }
  src/mcp.rs::SubmitSpecRequest: { source_hash: 7b1609e3e98dc3250a27134735440851618cc646bb9162e6b217ae4a188f727f, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: bc86d4a0883e0a7c1fbe5803ce6b7a9988ddae96c2995c9a95eb33611a1b8070 }
  src/mcp.rs::SubmitSpecResponse: { source_hash: 584a6af40bf1b9c0a263a3e47981f044c365e16027b30261f75938270501f6dd, deps_hash: af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262, spec_hash: 00d14c160aed17d6690549c1aafc995e82ecccf705508426112b4d3a611b661e }
  src/mcp.rs::CodeOwlServer: { source_hash: 6dc80bd04673a0a2369b3de94f7669ca697441eabf386a541be54fc7ec2920d0, deps_hash: ae4b23e58f6d7d6d97e474c46a1a53c91cb44c5af3c385df81b9b29c2ba9c4ba, spec_hash: 5542479f2ec1e3b0be52461127e8fc7bcb38afd5aef57c5cc39a7de56d8bb14f }
  src/mcp.rs::impl ServerHandler for CodeOwlServer: { source_hash: 41fe3a9bccacd44456e09064f9da6f0f2741bc8512e5d1ac28689f69619eb63e, deps_hash: f2a74fbfb4a010ecb6919af784ed653c44ffec4954808e198d753becbf1ee6a1, spec_hash: 85d169ebefa47f36acfa77c5fb2820164a6ebd14b777d930b7652205a8b86519 }
---
# src/mcp.rs
## Summary
This file is CodeOwl's whole MCP (Model Context Protocol) surface: every tool a connected AI coding session can call, plus the request and response shapes for each. It splits into two halves. The read tools (`get_symbol`, `get_source`, `get_callers`, `get_callees`, `get_spec`, `search_code`, `get_spec_coverage`) never write anything and never call a language model. The two write-driving tools (`get_next_spec_task` and `submit_spec`) are what the client's own model uses to fetch a piece of writing work, write the prose, and send it back to be saved; CodeOwl itself holds no model credentials and never generates text. `get_symbol`, `get_callers` and `get_callees` expose the structural graph: a declaration's record, which files import it, and what a file imports. `get_source` returns the real code behind a symbol or file, with flags saying whether the text was cut short and whether the file has changed since the index last saw it. `get_spec` reads a stored spec for a symbol, file, feature, folder or the whole system and reports whether it is current, stale or missing, without ever starting generation. `get_next_spec_task` works bottom-up, handing out a file's symbols, then the file, then any feature it hosts, or a whole-repo sweep for `system`, and returns an explicit done object when nothing is left, which is what lets the generate loop stop safely. `submit_spec` is the save side and routes by the shape of the id. `search_code` is a thin wrapper over embedded regex search. `get_spec_coverage` reports the full current, stale, missing and smelly breakdown with a prioritized, paged list of what to write next; a scoped report covers files and folders only and leaves features and the system spec out. The server keeps its graph in a cell the file watcher swaps as files change, and each request works from one consistent snapshot.

## `IdRequest`
`pub struct IdRequest`
### Summary
The input shape shared by several MCP tools: a single `id` naming the thing to look up, such as a symbol, a file, or a document.
### Behavior
Has one field, `id`, a string. A symbol's stable id (for example `lib/utils.ts::cn`) is always accepted. Some tools using this same input also accept a bare file path, and `get_spec` additionally accepts `feature:<slug>`, `rollup:<dir>` and `system`; each tool's own description says exactly which forms it takes. The field's doc comment becomes the description a connected agent sees in the tool's schema, so it carries no project milestone numbers. The struct is plain data, filled in from the request by the MCP library, and does no validation of its own.
### Depends on
- externals: rmcp

## `SourceRequest`
`pub struct SourceRequest`
### Summary
The input for the tool that returns real source code: which symbol or file to read, and how many extra lines of surrounding code to include.
### Behavior
`id` is a symbol's stable id (such as `lib/utils.ts::cn`) or a bare file path, the same naming `get_symbol` and `get_spec` use. `context_lines` is optional: it adds that many lines on each side of the returned span, for example to see the surrounding imports or the enclosing match arm. When omitted it is 0, and whatever is asked it is capped at 20. The cap and the default are applied by the tool that handles the request, not by this struct, which is plain data. A missing `context_lines` in the incoming request is accepted and treated as unset.
### Depends on
- (none)

## `SourceResponse`
`pub struct SourceResponse`
### Summary
What the source-reading tool sends back: the code text, where it came from, and two honesty flags that say whether the text is complete and whether it still matches the index.
### Behavior
`id` echoes the request. `file` is always a real repo-relative path, even when the request named a symbol. `lines` is the true full span covered, 1-based and inclusive, and stays accurate when the text was cut short, so a caller who needs the rest knows exactly what is missing. `source` is the code text. `truncated` is true when `source` was cut at the byte cap, in which case the caller should read the file directly or ask for less context; nothing is dropped without this flag. `graph_in_sync` is true when the file still hashes to what the graph recorded. False means the file changed since the last reindex (the watcher waits about 300 ms), so `lines` is the graph's recorded span read against the current text and may no longer point at the right place if the edit moved lines. Plain data returned as JSON.
### Depends on
- (none)

## `SearchRequest`
`pub struct SearchRequest`
### Summary
The input for the code search tool: a pattern to look for, and optional limits on where to look and how much to return.
### Behavior
`query` is a regular expression searched across the repo's source files. `path` optionally limits matches to a file or folder by true path boundary, so `lib` matches `lib/foo.ts` and never `library/foo.ts`; omitting it searches everything. `ignore_case` is optional, and omitted or false means case-sensitive. `context_lines` gives that many lines of context on each side of each match, default 0 and capped at 5 whatever is asked, because one call can return many matches and an unbounded multiplier would bring back the oversized-response problem the per-line text cap exists to prevent. `max_results` can only lower the match cap: the default is 200, and omitting it or giving a larger number leaves 200 in place. The defaults and caps are applied by the search code, not by this struct, which is plain data from the request.
### Depends on
- (none)

## `CoverageRequest`
`pub struct CoverageRequest`
### Summary
The input for the spec coverage tool: an optional folder to narrow the report to, and an optional position for paging through the pending list.
### Behavior
`scope` narrows the report to a directory prefix such as `lib`; omitting it covers the whole repo. A scoped report covers files and folder summaries only: features and the system spec are repo-wide concepts, so `spec::coverage` leaves them out of the report whenever a scope is given. `cursor` is where to resume the `pending` list: pass back the previous call's `next_cursor` exactly, or omit it or give 0 for the first page. Only `pending` is paged, because it is the one field that grows without limit (a 626-file repo produced a 95 KB list, over the MCP result limit). Every other field in the response (`current`, `stale`, `missing`, the per-kind and per-folder breakdowns, the top stale files, orphans) is always computed over the whole of what the scope covers regardless of `cursor`. Plain data from the request, no validation here.
### Depends on
- `src/spec.rs::by_kind` — crate::spec
- `src/spec.rs::by_module` — crate::spec
- `src/spec.rs::top_stale_by_impact` — crate::spec

## `OrphanedSpecResponse`
`pub struct OrphanedSpecResponse`
### Summary
How the coverage report describes one leftover spec whose subject no longer exists, such as a deleted file. These are for a person to delete, not for the generate command to rewrite.
### Behavior
Carries the same three facts as the internal `OrphanedSpec`. `id` names what is gone, using the coverage report's naming (`rollup:` and `feature:` prefixes). `kind` is `file`, `rollup`, `feature` or `symbol`. A `symbol` entry means a section inside a file spec that is otherwise still valid, whose function was deleted from the source; only that section is dead, and it removes itself the next time the file's spec is regenerated for any other reason. `path` is the orphaned document's own path for the first three kinds, and for a symbol it is the containing file's spec path, since there is no separate document to delete. This differs from a `pending` entry, which still has something in the graph to regenerate against. Plain data returned as JSON.
### Depends on
- (none)

## `OrphanedSpec> for OrphanedSpecResponse`
`impl From<crate::spec::OrphanedSpec> for OrphanedSpecResponse`
### Summary
Converts the internal record of a leftover spec into the form sent over the tool interface.
### Behavior
A field-by-field move: the `id`, `kind` and `path` strings from the `OrphanedSpec` become the same three fields of an `OrphanedSpecResponse`, with nothing transformed, validated or dropped. It takes ownership of the input, so no copying is needed. Its purpose is to keep the wire format a separate type from the internal one, so either can change without silently altering the other. It cannot fail.
### Depends on
- `src/spec.rs::OrphanedSpec` — crate::spec
- `src/mcp.rs::OrphanedSpecResponse` — same file

## `CoverageItem> for CoverageItemResponse`
`impl From<crate::spec::CoverageItem> for CoverageItemResponse`
### Summary
Converts one row of the internal coverage list into the form sent over the tool interface.
### Behavior
A field-by-field move of the six values `id`, `kind`, `status`, `fan_in`, `smells` and `generations` from a `CoverageItem` into a `CoverageItemResponse`, with nothing changed, filtered or computed. It takes ownership of the input, so the strings and the list move across without copying. Keeping the response a separate type means the format an agent sees can be changed on purpose and does not shift by accident when the internal type does. It cannot fail.
### Depends on
- `src/spec.rs::CoverageItem` — crate::spec
- `src/mcp.rs::CoverageItemResponse` — same file

## `CoverageItemResponse`
`pub struct CoverageItemResponse`
### Summary
One row of the coverage report as an agent sees it: a document that needs attention, with its status, how many files depend on it, any quality warnings, and what it would cost to fix.
### Behavior
`id` can be passed straight to `get_next_spec_task` or `get_spec`: a repo-relative file path, or `rollup:<dir>`, `feature:<slug>` or `system`. `kind` is `file`, `rollup`, `feature` or `system`. `status` is `missing`, `stale` or `current`, where `current` appears in the pending list only when `smells` is non-empty, meaning the document matches the code by hash but a deterministic quality check still flagged it. `fan_in` is how many other files import something from this one, always 0 for non-file kinds. `smells` lists the warnings found in the document's current text, such as `cop_out_phrase`, `suspiciously_short` or `identical_dependencies_across_symbols`; it is independent of `status`, since matching hashes never prove the prose was meaningful, and empty for a clean document. `generations` is the number of get-task and submit cycles this document still costs, one unit of a budget. For a file that is its uncovered, stale or smelly top-level symbols plus one for the file summary if that needs writing, so a single row can be worth 20 or more. For a folder summary, a feature or the system spec it is 1. Plain data returned as JSON.
### Depends on
- (none)

## `KindBreakdown`
`pub struct KindBreakdown`
### Summary
The coverage numbers for one type of document (files, folder summaries, features, or the system spec), in the same shape as the repo-wide totals, so an agent can read each type separately.
### Behavior
Fields: `kind` names the document type. `current`, `stale`, `missing` and `smelly` are the counts, and `generations_remaining` is the number of get-task and submit cycles still needed for this kind. `total` is `current + stale + missing`, so the feature row's total directly answers how many feature specs will ever exist. `freshness` is `current / (current + stale)` within this kind, the share of existing specs that still match the code. `coverage` is `(current + stale) / total` within this kind, the share that exist at all. The two ratios are separate on purpose: a kind can be fully written and mostly out of date, or mostly unwritten and all current. These values come from `by_kind` over the coverage list. Plain data returned as JSON.
### Depends on
- `src/spec.rs::coverage` — crate::spec
- `src/spec.rs::by_kind` — crate::spec

## `ModuleBreakdown`
`pub struct ModuleBreakdown`
### Summary
The coverage numbers for one folder, in the same shape as the per-type breakdown, so an agent can see how documented each part of the repo is.
### Behavior
Fields: `path` is the folder, and the counts `current`, `stale`, `missing`, `smelly`, `generations_remaining` and `total` mean the same as in `KindBreakdown`. `freshness` is `current / (current + stale)` within this folder and `coverage` is `(current + stale) / total` within it. A file's folder and its own folder summary share one group, so `rollup:lib/email` and `lib/email/foo.ts` both count under `lib/email`, which makes "what is left in this module" a single number and not two. Features and the system spec have no folder and never appear here. Folders are listed in path order, from `by_module`. Plain data returned as JSON.
### Depends on
- `src/spec.rs::coverage` — crate::spec

## `CoverageSummary)> for KindBreakdown`
`impl From<(String, crate::spec::CoverageSummary)> for KindBreakdown`
### Summary
Builds a per-type breakdown for the tool interface from a document type's name and its coverage summary.
### Behavior
Takes a pair of the kind name and a `CoverageSummary`. The five stored counts (`current`, `stale`, `missing`, `smelly`, `generations_remaining`) are copied across unchanged. The other three fields are computed by calling the summary's own methods: `total` from `total()`, `freshness` from `freshness()` and `coverage` from `coverage_ratio()`. Computing them through those methods means the ratios use the one shared definition, including the rule that a kind with nothing yet written reports freshness 1.0. It cannot fail.
### Depends on
- `src/spec.rs::CoverageSummary` — crate::spec
- `src/spec.rs::coverage` — crate::spec
- `src/mcp.rs::KindBreakdown` — same file

## `CoverageSummary)> for ModuleBreakdown`
`impl From<(String, crate::spec::CoverageSummary)> for ModuleBreakdown`
### Summary
Builds a per-folder breakdown for the tool interface from a folder path and its coverage summary.
### Behavior
Identical in logic to the per-type conversion, with the folder path in place of the kind name. It takes a pair of path and `CoverageSummary`, copies the five stored counts across unchanged, and computes `total`, `freshness` and `coverage` by calling the summary's `total()`, `freshness()` and `coverage_ratio()` methods, so folders and kinds use one shared definition of those ratios. It cannot fail.
### Depends on
- `src/spec.rs::CoverageSummary` — crate::spec
- `src/spec.rs::coverage` — crate::spec
- `src/mcp.rs::ModuleBreakdown` — same file

## `CoverageResponse`
`pub struct CoverageResponse`
### Summary
The full coverage report an agent receives: how many specs are current, stale or missing, how fresh and how complete the corpus is, which stale files matter most, which specs are leftovers, and the ordered list of what to write next.
### Behavior
Counts: `current`, `stale`, `missing`, and `smelly`, which counts documents of any status with at least one quality warning and so overlaps `current`. `generations_remaining` is the total get-task and submit cycles a full generate run would spend, summing each pending document's own cost, so it counts uncovered symbols and not just files: 15 missing files can mean 240 or more cycles.

Two separate ratios: `freshness` is `current / (current + stale)`, ignoring missing, and 1.0 when nothing has been generated; `coverage` is `(current + stale) / total`. They answer different questions: 100% coverage with 60% freshness means everything was written once and much needs re-running, while 60% coverage with 100% freshness means nothing written is wrong but more remains. `weighted_freshness` is freshness weighted by import fan-in, over file documents that have a spec, so a stale file forty others import hurts more than a stale leaf; with no fan-in anywhere it falls back to the plain ratio.

Breakdowns: `by_kind` splits the counts by document type (file, rollup, feature, system) in that order, skipping empty types, and its feature row's total is the full number of feature specs the repo will ever have. `by_module` splits by folder, sorted by path, with a folder's own summary and its files in one row. `top_stale_by_impact` is the five file documents most worth regenerating, by fan-in, to answer "what hurts most if left wrong" and not "what order to spend a budget on". `orphaned` lists spec documents whose subject is gone; they are not counted in any total and never appear in `pending`, since they are for deletion, not regeneration.

`pending` is every document still needing attention (not current, or current but smelly) in generation priority order, paged at a fixed size per call; `next_cursor` is the value to pass back for the next page and is `None` at the end. Every other field covers the whole scope regardless of paging. `generated_sources` reports how many files sit under the language's known build-generated folders and which folders were checked; it is `None` for languages with no such convention. A count of 0 where the convention exists is the actionable case, meaning the project may not have been built locally yet. Plain data returned as JSON.
### Depends on
- `src/spec.rs::coverage` — crate::spec
- `src/spec.rs::weighted_freshness` — crate::spec
- `src/spec.rs::by_kind` — crate::spec
- `src/spec.rs::by_module` — crate::spec
- `src/spec.rs::top_stale_by_impact` — crate::spec
- `src/mcp.rs::OrphanedSpecResponse` — same file
- `src/mcp.rs::CoverageItemResponse` — same file
- `src/mcp.rs::KindBreakdown` — same file
- `src/mcp.rs::ModuleBreakdown` — same file
- `src/mcp.rs::GeneratedSourcesResponse` — same file

## `GeneratedSourcesResponse`
`pub struct GeneratedSourcesResponse`
### Summary
The part of the coverage report that says how many files were found in the folders where a build tool puts generated code, so an agent can tell the user when a project probably needs building first.
### Behavior
`checked_dirs` lists the generated-source folders that were looked at (for example Maven's `target/generated-sources` and Gradle's `build/generated`), and `found` is the number of indexed files under them. The coverage report includes this only for languages that have such a convention. A `found` of 0 with a non-empty list is the useful signal: entry points defined only on generated code will not be visible until the project has been built. It is plain data returned as JSON, a copy of the internal `GeneratedSourcesSummary` with the same two fields.
### Depends on
- (none)

## `GeneratedSourcesSummary> for GeneratedSourcesResponse`
`impl From<crate::spec::GeneratedSourcesSummary> for GeneratedSourcesResponse`
### Summary
Converts the internal generated-sources report into the form sent over the tool interface.
### Behavior
A direct move of the two fields: the list of checked folders and the found count go into a `GeneratedSourcesResponse` unchanged. It takes ownership of the input, so the list is moved and not copied. Keeping the wire type separate means the format an agent sees does not change by accident when the internal type does. It cannot fail.
### Depends on
- `src/spec.rs::GeneratedSourcesSummary` — crate::spec
- `src/mcp.rs::GeneratedSourcesResponse` — same file

## `CallerInfo`
`pub struct CallerInfo`
### Summary
One entry in the list of who uses something: a file that imports it, and the name it imported.
### Behavior
Two fields: `from_file` is the repo-relative path of the importing file, and `imported_name` is the name it asked for. The caller list is built from the resolved import links in the graph, so each entry stands for one import statement of one name, and a file that imports three names from the same place appears three times. For a table the same shape is used to report a file that queries it, with the table name in `imported_name`. Plain data returned as JSON with no logic of its own.
### Depends on
- (none)

## `CallersResponse`
`pub struct CallersResponse`
### Summary
What the "who uses this" tool returns: the list of files that import the thing asked about.
### Behavior
Holds one field, `callers`, a list of `CallerInfo`. It is wrapped in a struct instead of being returned as a bare list because the MCP specification requires a tool's structured result to be a JSON object, and a compliant client rejects a top-level array. An empty list means nothing in the repo imports it, or that the imports did not resolve to it. Plain data returned as JSON with no logic.
### Depends on
- (none)

## `CalleeInfo`
`pub struct CalleeInfo`
### Summary
One entry in the list of what a file depends on: a name it imports, where from, and the matching declaration in this repo if one was found.
### Behavior
`specifier` is the module path as written in the import (`./foo`, `react`), and `imported_name` is the name it asked for. `resolved_id` is the stable id of the declaration it resolved to, or `None` when it did not resolve to anything CodeOwl tracks. That covers external packages, broken imports, and forms the resolver does not follow, such as a default or namespace import re-exported by a clause. TypeScript types, interfaces, enums and destructured constants are extracted as symbols, so they resolve like any other name. A `None` is therefore never benign by default: it may be an outside package or a real gap. Plain data returned as JSON.
### Depends on
- (none)

## `CalleesResponse`
`pub struct CalleesResponse`
### Summary
What the "what does this depend on" tool returns: the list of things a file imports.
### Behavior
Holds one field, `callees`, a list of `CalleeInfo`. As with the callers result, it is wrapped in a struct because the MCP specification requires a tool's structured result to be a JSON object, and a compliant client rejects a top-level array. An empty list means the file imports nothing that was recorded. Plain data returned as JSON with no logic.
### Depends on
- (none)

## `SearchResponse`
`pub struct SearchResponse`
### Summary
What the code search tool returns: the list of matches found, and a flag saying whether the search stopped early so some matches may be missing.
### Behavior
`matches` is the list of `SearchMatch` entries. `truncated` is true when either the match-count cap or the total response-size budget stopped the search before every real match in the repo was found. It is different from a single match's own `truncated` field, which only says that one match's text was cut short. To see more, narrow `path` or raise `max_results` (up to 200); the response-size budget is not adjustable by the caller. A false value means the search covered everything it was asked to. It is wrapped in a struct, not a bare list, because the MCP specification requires a tool's structured result to be a JSON object. Plain data returned as JSON.
### Depends on
- `src/search.rs::SearchMatch` — crate::search

## `SpecResponse`
`pub struct SpecResponse`
### Summary
What the spec-reading tool returns for one item: whether a spec exists and is still right, the written text itself, and, if it is out of date, exactly what changed.
### Behavior
`id` echoes the request. `status` is `missing` (nothing written yet), `current` (all inputs still match), or `stale` (a spec exists but at least one input has moved since it was written). The last known good text is always returned, even when stale, and only a missing spec has none. `signature` and `docstring` come from the graph, filled in by CodeOwl and not written by an agent. `content` is the agent-written prose, present for `current` and `stale` and `None` for `missing`. `changed` names which inputs moved, using fixed labels such as `source`, `changed:<dependency id>` and `added:<participant id>`, and is empty unless the status is `stale`; it is computed from hashes with no model call. `smells` lists quality warnings in the content, such as `cop_out_phrase` and `suspiciously_short`, and can be non-empty even when the status is `current`, because matching hashes only prove the inputs did not move and never that the writing is meaningful. It is always empty when the status is `missing`. Plain data returned as JSON.
### Depends on
- (none)

## `GenerateTaskRequest`
`pub struct GenerateTaskRequest`
### Summary
The input for the tool that hands out the next piece of spec-writing work: which target to work on.
### Behavior
Has one field, `target`, a string that can be a file path (such as `lib/utils.ts`, which may also be a feature entry point like a page or an API route nothing else references), a folder path with at least two documented files, or `system` or `.` for the whole repo, meaning every folder, every feature and finally the system spec. The tool is stateless, so it can be called again and again with the same target until it reports nothing left, which is how the generate command loops. The struct is plain data, filled in by the MCP library, and does no validation itself; an unknown target is handled by the tool that receives it.
### Depends on
- (none)

## `CoreSource`
`pub struct CoreSource`
### Summary
One file's full text, as handed to an agent writing a feature spec: the file's path and its source.
### Behavior
Two fields: `file` is the repo-relative path and `source` is the file's text. A feature task carries a list of these for the feature's own code, namely the entry point and the files reached from it, so the agent can read all of it before writing. The text may have been shortened by the size cap that keeps a task within what a client can accept. Plain data returned as JSON with no logic.
### Depends on
- (none)

## `DependencyContext`
`pub struct DependencyContext`
### Summary
A short description of one thing a feature depends on, so the agent writing the feature spec understands what it does without reading its code.
### Behavior
`id` is the dependency's stable symbol id. `summary` is the dependency's already-written summary if it has a current spec, and otherwise a plain stub made from its signature and docstring. Producing it never triggers generation of the dependency itself, so a feature task cannot cascade into writing other specs. Plain data returned as JSON with no logic of its own.
### Depends on
- (none)

## `TableContext`
`pub struct TableContext`
### Summary
One database table a feature touches, with its real column list, so the "Data touched" part of a feature spec can name actual columns instead of guessing from query code.
### Behavior
`id` is the table's schema node id, such as `supabase/schema.sql::payments`. `columns` is a text form `table(col, col, ...)` listing the column names parsed from the table's definition. Only names are listed, not types or foreign keys, because the schema parser records column names only. Plain data returned as JSON with no logic.
### Depends on
- (none)

## `RollupFile`
`pub struct RollupFile`
### Summary
One file's entry in a folder summary task: its path and the summary already written for it.
### Behavior
`file` is the repo-relative path and `summary` is that file's own current summary text, never its raw source, so a folder summary costs nothing beyond what was already generated. It is only filled in once the file's own spec is current, which the task-ordering code in `next_task_for_directory` guarantees by finishing every file in the folder before offering the folder summary. Plain data returned as JSON with no logic.
### Depends on
- `src/spec.rs::next_task_for_directory` — crate::spec

## `SpecTaskResponse`
`pub enum SpecTaskResponse`
### Summary
The six shapes of work the generate tool can hand an agent: write one function's spec, a file's summary, a feature, a folder summary, the whole-repo summary, or nothing because everything is current. The agent writes the text and sends it back, and CodeOwl never writes prose itself.
### Behavior
Each variant carries the context for that kind of writing and is sent as JSON tagged with its kind.
- `Symbol`: the symbol's id, signature, docstring, source text and dependency list, plus `prior_summary` and `prior_behavior`. Those two are set only for a reconciliation rewrite, where the code moved and a human had also edited the prose, so the agent can keep what is still accurate and not overwrite the correction.
- `File`: the file's id, its source, and `prior`, the human-edited summary in the same reconciliation case.
- `Feature`: written in one go because a feature is a single document with a single fingerprint. Its `id` is `feature:<slug>` and should be sent straight back as the submit id. It carries the entry point, the full text of the core files, a summary or stub for each dependency, and the database tables the code queries with their real column lists, so the data section can name actual columns.
- `Rollup`: a folder summary, with id `rollup:<dir>` and each file's already-written summary, never raw source. It is only offered once every file in the folder is current.
- `System`: the one whole-repo document, with id `system`, built from every module's and every feature's summary, and offered only once all of those are current.
- `Done`: nothing left for the target. It is serialised as `{"kind":"done"}`, a real object, because some MCP clients reject a bare `null` as invalid structured content. It is the generate loop's stop signal.

The enum is plain data built by the tool handler from the spec module's task types.
### Depends on
- (none)

## `ModuleSummary`
`pub struct ModuleSummary`
### Summary
One module folder's entry in the whole-repo summary task: the folder's path and the summary already written for it.
### Behavior
`dir` is the repo-relative folder path and `summary` is that folder's own current summary text. The system task carries a list of these, one per module, so the agent writes the top-level description from existing summaries and not from code. The list is only built once every module's summary is current. Plain data returned as JSON with no logic.
### Depends on
- (none)

## `FeatureSummary`
`pub struct FeatureSummary`
### Summary
One feature's entry in the whole-repo summary task: its short name and its title and summary, already written.
### Behavior
`slug` is the feature's short name and `summary` combines the feature's own title with its current summary text. The system task carries a list of these next to the module summaries, so the agent can describe the product from what exists. For a repo with no features, such as a command-line tool, this list is empty. Plain data returned as JSON with no logic.
### Depends on
- (none)

## `SubmitSpecRequest`
`pub struct SubmitSpecRequest`
### Summary
The input for the tool that saves a spec: which item the text is for, and the text itself.
### Behavior
`id` is exactly the id the task tool returned: a symbol id, a file id, the fixed id `system`, or `feature:<slug>` or `rollup:<dir_path>` for those tasks. `content` depends on the kind. For a symbol it is markdown containing `### Summary` and `### Behavior` headings. For a file or a folder summary it is plain prose that becomes the `## Summary`. For a feature or the system spec it is the whole document starting with a `# Title` line. The struct is plain data, and the checks (headings present, text long enough, no evasive phrases) are made by the submit tool, which rejects content that fails them and writes nothing.
### Depends on
- (none)

## `SubmitSpecResponse`
`pub struct SubmitSpecResponse`
### Summary
What the save tool confirms back: which item was saved and the fingerprints recorded for it.
### Behavior
`id` echoes the request. `spec_hash` is the fingerprint of the saved prose. `source_hash` is the fingerprint of the code the spec was written from, for a symbol or a file. It is `None` for a feature, folder summary or system spec, because none of those has one source hash: a feature records a map of its participants, a folder summary records a hash per file, and the system spec records both a module map and a feature map, all of which live in the saved document's frontmatter. The response only reports and the saved document is what staleness is checked against. Plain data returned as JSON.
### Depends on
- (none)

## `CodeOwlServer`
`pub struct CodeOwlServer`
### Summary
The MCP server itself: it holds the live index of the repo and exposes the nine tools an AI coding session can call (look up a symbol, read source, find callers and dependencies, read a spec, ask for the next spec-writing task, save a spec, search code, and see spec coverage). It never writes prose or calls a language model; the connected agent does the writing and sends it back.
### Behavior
State: `graph` is a shared, swappable cell holding the current graph, which the file watcher replaces whole as source files change. Every request loads one consistent snapshot up front and never re-reads the cell mid-request, since an arena id is only valid for the graph that produced it. `root` is the repo path. `tool_router` dispatches tool calls. Three size limits are fields, not environment variables, so the active values are visible where `codeowl serve` is invoked: `large_class_bytes` (when a class is shown to an agent as an outline instead of in full), `max_spec_task_bytes` (hard cap on task text), and `max_source_bytes` (cap on the source read tool, deliberately separate and larger, since it is a verification read and not a prompt). Builder methods `with_generation_limits` and `with_source_limit` override defaults only for the values given, and `graph_store` hands the shared cell to the watcher.

Tools: `get_symbol` returns a symbol's full record, with an error for an unknown id. `get_source` reads the real text from disk for a symbol (all its line ranges, including folded `impl` blocks) or a whole file. It adds context lines capped at 20, clamps the reported lines to what the file currently has, applies the byte cap and sets `truncated`, and sets `graph_in_sync` by comparing the file's current hash with the recorded one, using the file's own hash and not a container's folded hash, which could never match. `get_callers` lists importers of a symbol, or for a table node the files whose query resolved to it; a bare file id gives an empty list. `get_callees` lists what the owning file imports, with the resolved symbol id or none. `get_spec` is a pure read: the id is routed by shape to `system`, `rollup:`, `feature:`, or a file or symbol, returning `missing`, `current` or `stale` with the last good text, what moved, and quality warnings.

`get_next_spec_task` hands out the next unit of work. `system` or `.` walks every module and every feature before offering the one system task, a `feature:<slug>` goes to that exact entry and never a sibling sharing its file (an earlier bug), a `rollup:` or folder path walks its files then the folder summary, and a file or symbol id walks symbols, then the file, then any features that file hosts, trying each in turn until one is not current. A symbol task carries the span text, scoped dependencies computed from the full text, and, for a very large class, an outline and the size cap. Nothing left is returned as an explicit `Done` object, because some clients reject a bare null. `submit_spec` routes the id by the same shapes to the matching save function and returns the hashes recorded, with no source hash for feature, folder and system specs. `search_code` wraps the embedded regex search. `get_spec_coverage` builds the coverage list, summary, breakdowns, five most important stale files, orphans, the prioritized pending list paged at a fixed size, and the generated-source report. `get_info` supplies the server's instructions text.
### Depends on
- `src/graph.rs::Graph` — crate::graph
- `src/graph.rs::Node` — crate::graph
- `src/graph.rs::SymbolView` — crate::graph
- `src/symbol.rs::SymbolKind` — crate::symbol
- `src/spec.rs::LARGE_CONTAINER_BYTES_DEFAULT` — crate::spec
- `src/spec.rs::MAX_GENERATION_TASK_TEXT_BYTES_DEFAULT` — crate::spec
- `src/spec.rs::MAX_SOURCE_TEXT_BYTES_DEFAULT` — crate::spec
- `src/features.rs::enumerate_entry_points` — crate::features
- `src/features.rs::EntryPoint` — crate::features
- `src/spec.rs::next_task` — crate::spec
- `src/spec.rs::next_feature_task` — crate::spec
- `src/spec.rs::FeatureTask` — crate::spec
- `src/spec.rs::directory_is_spec_bearing` — crate::spec
- `src/spec.rs::next_task_for_directory` — crate::spec
- `src/spec.rs::next_rollup_task` — crate::spec
- `src/spec.rs::SpecTask` — crate::spec
- `src/spec.rs::symbol_span_text` — crate::spec
- `src/spec.rs::scoped_symbol_deps` — crate::spec
- `src/spec.rs::maybe_reduce_container_source` — crate::spec
- `src/spec.rs::cap_generation_text` — crate::spec
- `src/spec.rs::merged_symbol_spans` — crate::spec
- `src/hash.rs::hash_text` — crate::hash
- `src/spec.rs::render_spans` — crate::spec
- `src/spec.rs::cap_source_text` — crate::spec
- `src/spec.rs::read_system_spec` — crate::spec
- `src/spec.rs::body_smells` — crate::spec
- `src/spec.rs::current_module_hashes` — crate::spec
- `src/spec.rs::current_feature_hashes` — crate::spec
- `src/spec.rs::diff_hash_lists` — crate::spec
- `src/spec.rs::read_rollup_spec` — crate::spec
- `src/spec.rs::current_file_hashes` — crate::spec
- `src/spec.rs::read_feature_spec` — crate::spec
- `src/features.rs::feature_model_for` — crate::features
- `src/features.rs::assemble_participants` — crate::features
- `src/spec.rs::current_participant_hashes` — crate::spec
- `src/spec.rs::read_file_spec` — crate::spec
- `src/spec.rs::symbol_changes` — crate::spec
- `src/spec.rs::prose_smells` — crate::spec
- `src/spec.rs::file_changes` — crate::spec
- `src/spec.rs::file_spec_smells` — crate::spec
- `src/spec.rs::enumerate_modules` — crate::spec
- `src/spec.rs::next_system_task` — crate::spec
- `src/spec.rs::submit_system` — crate::spec
- `src/spec.rs::submit_rollup` — crate::spec
- `src/spec.rs::submit_feature` — crate::spec
- `src/spec.rs::submit` — crate::spec
- `src/search.rs::SearchOptions` — crate::search
- `src/search.rs::search_code` — crate::search
- `src/spec.rs::coverage` — crate::spec
- `src/spec.rs::summarize` — crate::spec
- `src/spec.rs::weighted_freshness` — crate::spec
- `src/spec.rs::by_kind` — crate::spec
- `src/spec.rs::by_module` — crate::spec
- `src/spec.rs::top_stale_by_impact` — crate::spec
- `src/spec.rs::find_orphaned_specs` — crate::spec
- `src/spec.rs::prioritize` — crate::spec
- `src/spec.rs::generated_sources_summary` — crate::spec
- `src/mcp.rs::OrphanedSpecResponse` — same file
- `src/mcp.rs::CoverageItemResponse` — same file
- `src/mcp.rs::KindBreakdown` — same file
- `src/mcp.rs::ModuleBreakdown` — same file
- `src/mcp.rs::GeneratedSourcesResponse` — same file
- externals: arc_swap, rmcp, std

## `impl ServerHandler for CodeOwlServer`
`impl ServerHandler for CodeOwlServer`
### Summary
Tells a connecting AI client what this server is and how to use it: it announces that tools are available and gives a short guide to when to reach for each one.
### Behavior
`get_info` builds the server's description. Because the library's info type cannot be constructed with a plain struct literal, it starts from the default and sets fields one by one: the newest protocol version the library supports, a capabilities object with only tools enabled, and a fixed instructions string. The instructions say that CodeOwl is a read-only structural index to try before grepping on "how is this wired" questions, summarise `get_symbol`, `get_callers`, `get_callees` and the plain-regex `search_code`, explain that SQL tables appear as table nodes, that the index follows the working tree within about a second, how `get_spec` reports missing, stale and smelly specs, that `get_spec_coverage` lists what needs attention, and that writing specs goes through the `/codeowl-generate` command and its task-and-submit loop. The tool-calling itself is wired by the attribute on the impl, which routes calls to the tool router. The text is a constant and is not generated from the tool list, so it must be updated by hand when a tool is added.
### Depends on
- `src/search.rs::search_code` — crate::search
- `src/mcp.rs::CodeOwlServer` — same file
- externals: rmcp
