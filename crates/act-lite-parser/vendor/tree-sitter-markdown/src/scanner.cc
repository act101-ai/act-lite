#include <tree_sitter/parser.h>
#include <cassert>
#include <list>
#include <vector>

#include "./tree_sitter_markdown/token_type.h"

#ifdef TREE_SITTER_MARKDOWN_AVOID_CRASH
#define TREE_SITTER_MARKDOWN_ASSERT(condition) if (!(condition)) throw 1;
#else
#define TREE_SITTER_MARKDOWN_ASSERT(condition) assert(condition)
#endif

// tree-sitter does not support multiple files for external scanner
#include "./tree_sitter_markdown/block_context.cc"
#include "./tree_sitter_markdown/block_delimiter.cc"
#include "./tree_sitter_markdown/block_scan.cc"
#include "./tree_sitter_markdown/block_scan_util.generated.cc"
#include "./tree_sitter_markdown/inline_delimiter.cc"
#include "./tree_sitter_markdown/inline_context.cc"
#include "./tree_sitter_markdown/inline_scan.cc"
#include "./tree_sitter_markdown/inline_scan_util.generated.cc"
#include "./tree_sitter_markdown/lexer.cc"
#include "./tree_sitter_markdown/parse_table.cc"
#include "./tree_sitter_markdown/predicate.cc"
#include "./tree_sitter_markdown/predicate_util.generated.cc"
#include "./tree_sitter_markdown/shared_type.cc"
#include "./tree_sitter_markdown/util.cc"

namespace {

using namespace tree_sitter_markdown;

struct Scanner {
  Lexer lxr_;
  MinimizedInlineDelimiterList min_inl_dlms_;
  InlineDelimiterList inl_dlms_;
  InlineContextStack inl_ctx_stk_;
  BlockDelimiterList blk_dlms_;
  BlockContextStack blk_ctx_stk_;
  bool has_opt_wsp_ind_;

  Scanner() {
    deserialize(NULL, 0);
  }

  unsigned serialize(unsigned char *buffer) {
    size_t expected_size = lxr_.serialized_size();
    expected_size += min_inl_dlms_.serialized_size();
    expected_size += blk_dlms_.serialized_size();
    expected_size += blk_ctx_stk_.serialized_size();
    expected_size += 1;
    if (expected_size > TREE_SITTER_SERIALIZATION_BUFFER_SIZE) return 0;

    size_t i = 0;

    i += lxr_.serialize(&buffer[i]);
    i += min_inl_dlms_.serialize(&buffer[i]);
    i += blk_dlms_.serialize(&buffer[i]);
    i += blk_ctx_stk_.serialize(&buffer[i]);
    buffer[i++] = has_opt_wsp_ind_;

    TREE_SITTER_MARKDOWN_ASSERT(i == expected_size);

    return i;
  }

  void deserialize(const unsigned char *buffer, unsigned length) {
    lxr_.clear();
    min_inl_dlms_.clear();
    inl_dlms_.clear();
    inl_ctx_stk_.clear();
    blk_dlms_.clear();
    blk_ctx_stk_.clear();
    has_opt_wsp_ind_ = false;

    if (length > 0) {
      size_t i = 0;

      i += lxr_.deserialize(&buffer[i]);
      i += min_inl_dlms_.deserialize(&buffer[i]);
      i += blk_dlms_.deserialize(&buffer[i]);
      i += blk_ctx_stk_.deserialize(&buffer[i]);
      has_opt_wsp_ind_ = buffer[i++];

      TREE_SITTER_MARKDOWN_ASSERT(i == length);
    }
  }

  bool scan(TSLexer *lexer, const bool *valid_symbols) {
    lxr_.init(lexer);
    lxr_.mrk_end();

    if (!min_inl_dlms_.empty() && is_inl_cls_mrk_sym(min_inl_dlms_.front().sym())) {
      TREE_SITTER_MARKDOWN_ASSERT(min_inl_dlms_.front().len() == 0);
      TokenType rlt_sym = min_inl_dlms_.front().tkn_typ(lxr_.cur_chr(), lxr_.lka_chr());
      if (rlt_sym != TKN_NOT_FOUND) {
        min_inl_dlms_.pop_front();
        has_opt_wsp_ind_ = false;
        return lxr_.ret_sym(rlt_sym);
      }
    }

    if (
      !blk_dlms_.empty()
      && (blk_dlms_.front().sym() == SYM_VRT_SPC
        || blk_dlms_.front().sym() == SYM_IND_COD_BGN_PFX
        || blk_dlms_.front().sym() == SYM_IND_COD_BGN_MKR
        || blk_dlms_.front().sym() == SYM_LST_ITM_CNT_BGN_MKR)
    ) {
      BlockDelimiter &dlm = blk_dlms_.front();
      TokenType rlt_sym = dlm.tkn_typ(lxr_.lka_chr());
      if (rlt_sym != TKN_NOT_FOUND) {
        if (dlm.sym() == SYM_VRT_SPC) {
          if (dlm.len() == 1) {
            blk_dlms_.pop_front();
          } else {
            dlm.set_len(dlm.len() - 1);
          }
        } else if (dlm.sym() == SYM_IND_COD_BGN_PFX) {
          lxr_.adv_len(dlm.len());
          blk_dlms_.pop_front();
        } else {
          blk_ctx_stk_.push(BlockContext(dlm.sym(), dlm.len(), dlm.ind()));
          blk_dlms_.pop_front();
        }
        lxr_.mrk_end();
        has_opt_wsp_ind_ = false;
        return lxr_.ret_sym(rlt_sym);
      }
    }

    bool has_wsp = lxr_.adv_rpt(is_wsp_chr);

    if (has_wsp && is_eol_chr(lxr_.lka_chr()) && valid_symbols[TKN_TXT] && valid_symbols[TKN_BLK_LBK]) {
      lxr_.mrk_end();
      return lxr_.ret_sym(TKN_TXT); // BLK_TXT
    }

    if (!blk_dlms_.empty()) {
      BlockDelimiter &dlm = blk_dlms_.front();
      TokenType rlt_sym = dlm.tkn_typ(lxr_.lka_chr());
      if (rlt_sym != TKN_NOT_FOUND) {
        // whitespaces are not considered part of block token
        if (has_wsp && !/*exception*/(rlt_sym == TKN_LIT_LBK || rlt_sym == TKN_BNK_LBK || rlt_sym == TKN_FEN_COD_CTN_BGN_MKR)) {
          lxr_.mrk_end();
          // has_opt_wsp_ind_ is not affected
          return lxr_.ret_sym(TKN_WSP);
        }
        LexedLength spc_cnt = lxr_.cur_spc();
        if (dlm.len() || rlt_sym == TKN_PGH_BGN_MKR) {
          lxr_.adv_len(dlm.len());
          lxr_.mrk_end();
        }
        if (is_blk_opn_sym(dlm.sym())) {
          blk_ctx_stk_.push(BlockContext(dlm.sym(), dlm.len(), dlm.ind()));
          if (is_lst_itm_bgn(dlm.sym()) || dlm.sym() == SYM_BQT_BGN) {
            has_opt_wsp_ind_ = is_wsp_chr(lxr_.lka_chr());
          } else {
            has_opt_wsp_ind_ = false;
          }
        } else if (is_blk_cls_sym(dlm.sym())) {
          TREE_SITTER_MARKDOWN_ASSERT(is_paired_blk_syms(blk_ctx_stk_.back().sym(), dlm.sym()));
          blk_ctx_stk_.pop();
          has_opt_wsp_ind_ = false;
        } else {
          has_opt_wsp_ind_ = false;
        }
        blk_dlms_.pop_front();
        if (rlt_sym == TKN_LIT_LBK) {
          if (valid_symbols[TKN_HRD_LBK] && spc_cnt >= 2) rlt_sym = TKN_HRD_LBK;
          else if (valid_symbols[TKN_SFT_LBK]) rlt_sym = TKN_SFT_LBK;
          if (rlt_sym != TKN_LIT_LBK) {
            lxr_.adv_rpt(is_wsp_chr);
            lxr_.mrk_end();
          }
        }
        if (rlt_sym == TKN_LIT_LBK || rlt_sym == TKN_BNK_LBK) {
          if (valid_symbols[TKN_BLK_LBK]) rlt_sym = TKN_BLK_LBK;
        }
        return lxr_.ret_sym(rlt_sym);
      }
    }

    if (!min_inl_dlms_.empty()) {
      if (has_wsp && !is_eol_chr(lxr_.lka_chr())) {
        lxr_.mrk_end();
        return lxr_.ret_sym(valid_symbols[TKN_TXT] ? TKN_TXT : TKN_WSP);
      }

      bool has_txt = false;
      while (!min_inl_dlms_.empty() && !is_eol_chr(lxr_.lka_chr())) {
        if (is_wht_chr(lxr_.lka_chr()) && valid_symbols[TKN_WRD]) {
          TREE_SITTER_MARKDOWN_ASSERT(has_txt);
          break;
        }

        MinimizedInlineDelimiter &dlm = min_inl_dlms_.front();
        TokenType rlt_sym = dlm.tkn_typ(lxr_.cur_chr(), lxr_.lka_chr());

        if (rlt_sym == TKN_NOT_FOUND) {
          if (is_wsp_chr(lxr_.lka_chr())) {
            TREE_SITTER_MARKDOWN_ASSERT(has_txt);
            lxr_.mrk_end();
            lxr_.adv_rpt(is_wsp_chr);
          } else {
            has_txt = true;
            lxr_.adv();
          }
          continue;
        }

        if (rlt_sym == TKN_TXT || !dlm.yes()) {
          lxr_.adv_len(dlm.len());
          min_inl_dlms_.pop_front();
          has_txt = true;
          continue;
        }

        if (has_txt) break;

        lxr_.adv_len(dlm.len());
        min_inl_dlms_.pop_front();

        if (rlt_sym == TKN_HRD_LBK) {
          TREE_SITTER_MARKDOWN_ASSERT(blk_dlms_.front().sym() == SYM_LIT_LBK);
          lxr_.adv_len(blk_dlms_.front().len());
          blk_dlms_.pop_front();
        }

        lxr_.mrk_end();
        has_opt_wsp_ind_ = false;
        return lxr_.ret_sym(rlt_sym);
      }

      has_opt_wsp_ind_ = false;

      if (is_eol_chr(lxr_.lka_chr()) && !has_txt) {
        lxr_.mrk_end();
        return lxr_.ret_sym(TKN_WSP);
      }

      TREE_SITTER_MARKDOWN_ASSERT(has_txt);
      if (!is_wsp_chr(lxr_.cur_chr())) lxr_.mrk_end();
      return lxr_.ret_sym(valid_symbols[TKN_WRD] ? TKN_WRD : TKN_TXT);
    }

    TREE_SITTER_MARKDOWN_ASSERT(min_inl_dlms_.empty());
    TREE_SITTER_MARKDOWN_ASSERT(inl_dlms_.empty());
    TREE_SITTER_MARKDOWN_ASSERT(inl_ctx_stk_.empty());

    if (blk_ctx_stk_.empty() && is_eof_chr(lxr_.lka_chr())) {
      TREE_SITTER_MARKDOWN_ASSERT(blk_dlms_.empty());
      if (valid_symbols[TKN_EOF]) return lxr_.ret_sym(TKN_EOF);
      return false;
    }

    if (is_eol_chr(lxr_.lka_chr())) {
      TREE_SITTER_MARKDOWN_ASSERT(blk_dlms_.empty());
      scn_eol(lxr_, blk_dlms_, blk_ctx_stk_);
      TREE_SITTER_MARKDOWN_ASSERT(!blk_dlms_.empty());
      return lxr_.ret_sym(TKN_LKA);
    }

    if (valid_symbols[TKN_IND_COD_BGN_PFX] || valid_symbols[TKN_LST_ITM_CNT_BGN_MKR]) {
      TREE_SITTER_MARKDOWN_ASSERT(blk_dlms_.empty());
      scn_blk(lxr_, blk_dlms_, blk_ctx_stk_, lxr_.cur_ind() - has_opt_wsp_ind_);
      TREE_SITTER_MARKDOWN_ASSERT(!blk_dlms_.empty());
      return lxr_.ret_sym(TKN_LKA);
    }

    lxr_.bgn_buf();

    Symbol sym = scn_inl(lxr_, inl_dlms_, inl_ctx_stk_, blk_dlms_, blk_ctx_stk_);
    if (sym == SYM_TXT) {
      do lxr_.mrk_end();
      while (!is_wht_chr(lxr_.lka_chr()) && scn_inl(lxr_, inl_dlms_, inl_ctx_stk_, blk_dlms_, blk_ctx_stk_) == SYM_TXT);
    } else if (sym != SYM_BLK_TXT) {
      TREE_SITTER_MARKDOWN_ASSERT(!inl_dlms_.empty());
    }

    inl_dlms_.transfer_to(min_inl_dlms_);

    if (sym == SYM_TXT || sym == SYM_BLK_TXT) {
      has_opt_wsp_ind_ = false;
      return lxr_.ret_sym(TKN_TXT);
    }

    return lxr_.ret_sym(TKN_LKA);
  }
};

}

extern "C" {

void *tree_sitter_markdown_external_scanner_create() {
  return new Scanner();
}

void tree_sitter_markdown_external_scanner_destroy(void *payload) {
  Scanner *scanner = static_cast<Scanner *>(payload);
  delete scanner;
}

unsigned tree_sitter_markdown_external_scanner_serialize(void *payload, char *buffer) {
  Scanner *scanner = static_cast<Scanner *>(payload);
#ifdef TREE_SITTER_MARKDOWN_AVOID_CRASH
  try {
    return scanner->serialize((unsigned char *)buffer);
  } catch (...) {
    // Buffer overflow on a pathological state (TREE_SITTER_MARKDOWN_ASSERT
    // in Scanner::serialize) — report an empty serialized state instead of
    // letting the C++ exception escape into Rust across the C tree-sitter
    // frames, which aborts the process ("Rust cannot catch foreign
    // exceptions"). A 0-length buffer is what Scanner::deserialize already
    // treats as "nothing to restore".
    return 0;
  }
#else
  return scanner->serialize((unsigned char *)buffer);
#endif
}

void tree_sitter_markdown_external_scanner_deserialize(void *payload, const char *buffer, unsigned length) {
  Scanner *scanner = static_cast<Scanner *>(payload);
#ifdef TREE_SITTER_MARKDOWN_AVOID_CRASH
  try {
    scanner->deserialize((unsigned char *)buffer, length);
  } catch (...) {
    // Serialized-state roundtrip mismatch (TREE_SITTER_MARKDOWN_ASSERT(i ==
    // length) in Scanner::deserialize) — same foreign-exception hazard as
    // above. By the time this assert fires, every sub-component
    // (lxr_/min_inl_dlms_/blk_dlms_/blk_ctx_stk_/has_opt_wsp_ind_) has
    // already been overwritten from `buffer` — the assert only checks that
    // the bytes consumed add up to `length`, it does not gate whether the
    // writes happened. So the scanner is already left exactly as it would
    // be with TREE_SITTER_MARKDOWN_ASSERT compiled to a no-op `assert()`
    // under NDEBUG (this grammar's normal non-AVOID_CRASH behavior) —
    // intentionally not re-clearing/resetting here: on this file's
    // GLR-ambiguous inline/table markup, resetting to a blank scanner on
    // every mismatch defeats the parser's incremental-state reuse and
    // turns a sub-second parse into a 30s ParseTimeout (verified while
    // fixing this bug), because the scanner keeps re-losing its nested
    // block/list context and re-triggering the same mismatch.
    //
    // Known hazard, deliberately inherited from upstream: a wrapped count
    // byte makes the mismatching component under-consume, so the following
    // components interpret mid-entry bytes as counts — each up to 255
    // entries with no bound against `length` — and can read past the end of
    // the buffer by a bounded amount (worst case ~1 KB). The read side is
    // left as-is because every alternative tried so far (clamping,
    // truncating, or dropping the over-bound component) changes the
    // restored state the parser relies on to terminate the
    // pathological-table-row parse; any future read-bounds hardening must
    // re-validate against that 30s ParseTimeout regression.
  }
#else
  scanner->deserialize((unsigned char *)buffer, length);
#endif
}

bool tree_sitter_markdown_external_scanner_scan(void *payload, TSLexer *lexer, const bool *valid_symbols) {
  Scanner *scanner = static_cast<Scanner *>(payload);
#ifdef TREE_SITTER_MARKDOWN_AVOID_CRASH
  try {
    return scanner->scan(lexer, valid_symbols);
  } catch (...) {
    return false;
  }
#else
  return scanner->scan(lexer, valid_symbols);
#endif
}

}
