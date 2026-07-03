package application

import (
	"fmt"
	"strings"
	"time"

	"github.com/aitrack/server/internal/domain/model"
	"github.com/aitrack/server/internal/domain/port"
	"github.com/aitrack/server/internal/domain/service"
)

const (
	maxStoredDiffHunkChars = 8192
	maxStoredMetadataChars = 4096
	maxStoredPromptChars   = 4096
)

// IngestService processes a batch of edits through the validation chain and persists them.
type IngestService struct {
	validation *service.ValidationService
	validator  *service.EditValidator
	editRepo   port.EditRecordPort
}

// NewIngestService constructs the ingest use case.
func NewIngestService(v *service.ValidationService, ev *service.EditValidator, repo port.EditRecordPort) *IngestService {
	return &IngestService{validation: v, validator: ev, editRepo: repo}
}

// Ingest validates and persists a batch, returning the per-index verdict.
func (s *IngestService) Ingest(token *model.Token, req *model.EditBatchRequest) *model.EditBatchResponse {
	resp := &model.EditBatchResponse{
		Rejected: []model.IndexedReason{},
		Flagged:  []model.IndexedReason{},
	}
	savedAny := false

	for i, edit := range req.Edits {
		editCopy := edit

		// Guard: explicit field validation (prevents panic from nil pointer dereference)
		if reason := s.validator.Validate(&editCopy); reason != "" {
			resp.Rejected = append(resp.Rejected, model.IndexedReason{Index: i, Reason: reason})
			continue
		}

		result := s.validation.Validate(token, &editCopy)
		switch result.Outcome {
		case service.OutcomeRejected:
			resp.Rejected = append(resp.Rejected, model.IndexedReason{
				Index:  i,
				Reason: strings.Join(result.Reasons, ","),
			})
		case service.OutcomeFlagged:
			if err := s.saveEdit(token, &editCopy, "FLAGGED", result.Reasons); err != nil {
				resp.Rejected = append(resp.Rejected, model.IndexedReason{
					Index:  i,
					Reason: fmt.Sprintf("save_error: %v", err),
				})
				continue
			}
			savedAny = true
			resp.Flagged = append(resp.Flagged, model.IndexedReason{
				Index:  i,
				Reason: strings.Join(result.Reasons, ","),
			})
		case service.OutcomeAccepted:
			if err := s.saveEdit(token, &editCopy, "ACCEPTED", nil); err != nil {
				resp.Rejected = append(resp.Rejected, model.IndexedReason{
					Index:  i,
					Reason: fmt.Sprintf("save_error: %v", err),
				})
				continue
			}
			savedAny = true
			resp.Accepted++
		}
	}

	if savedAny {
		if err := s.applyEditRawRetention(); err != nil {
			resp.Rejected = append(resp.Rejected, model.IndexedReason{
				Index:  -1,
				Reason: fmt.Sprintf("retention_error: %v", err),
			})
		}
	}

	return resp
}

func (s *IngestService) saveEdit(token *model.Token, edit *model.EditDTO, status string, flags []string) error {
	rec := &model.EditRecord{
		TokenKey:     token.TokenKey,
		DeviceID:     edit.DeviceID,
		Hostname:     edit.Hostname,
		Tool:         edit.Tool,
		ToolVersion:  edit.ToolVersion,
		Provider:     edit.Provider,
		SessionID:    edit.SessionID,
		RepoURL:      edit.RepoURL,
		Branch:       edit.Branch,
		CurrentSHA:   edit.CurrentSHA,
		FilePath:     edit.FilePath,
		AddedLines:   *edit.AddedLines,
		RemovedLines: *edit.RemovedLines,
		Timestamp:    edit.Timestamp,
		RecordSig:    edit.RecordSig,
		Status:       status,
		ReceivedAt:   time.Now().UTC(),
	}
	if edit.Model != nil {
		rec.Model = *edit.Model
	}
	if edit.DiffHunk != nil {
		rec.DiffHunk = truncateRunes(*edit.DiffHunk, maxStoredDiffHunkChars)
	}
	if edit.Metadata != nil {
		rec.Metadata = truncateRunes(*edit.Metadata, maxStoredMetadataChars)
	}
	if edit.PromptSummary != nil {
		promptSummary := truncateRunes(*edit.PromptSummary, maxStoredPromptChars)
		rec.PromptSummary = &promptSummary
	}
	if len(flags) > 0 {
		rec.Flags = strings.Join(flags, ",")
	}
	return s.editRepo.Save(rec)
}

func (s *IngestService) applyEditRawRetention() error {
	retentionRepo, ok := s.editRepo.(port.EditRecordRetentionPort)
	if !ok {
		return nil
	}
	_, err := retentionRepo.ApplyRawRetention(time.Now().UTC())
	return err
}

func truncateRunes(value string, maxRunes int) string {
	runes := []rune(value)
	if len(runes) <= maxRunes {
		return value
	}
	return string(runes[:maxRunes])
}

// QueryEdits returns a paginated list of stored edit records.
func (s *IngestService) QueryEdits(tokenKey, repoURL string, page, size int) (*model.EditQueryResult, error) {
	records, total, err := s.editRepo.Query(tokenKey, repoURL, page, size)
	if err != nil {
		return nil, err
	}
	if records == nil {
		records = []model.EditRecord{}
	}
	return &model.EditQueryResult{
		Total:   total,
		Page:    page,
		Size:    size,
		Records: records,
	}, nil
}
