package service

import (
	"strings"
	"time"

	"github.com/aitrack/server/internal/model"
)

// IngestService processes a batch of edits through the validation chain and persists them.
type IngestService struct {
	validation *ValidationService
	validator  *EditValidator
	editRepo   *EditRecordRepository
}

func NewIngestService(v *ValidationService, ev *EditValidator, repo *EditRecordRepository) *IngestService {
	return &IngestService{validation: v, validator: ev, editRepo: repo}
}

func (s *IngestService) Ingest(token *model.Token, req *model.EditBatchRequest) *model.EditBatchResponse {
	resp := &model.EditBatchResponse{
		Rejected: []model.IndexedReason{},
		Flagged:  []model.IndexedReason{},
	}

	for i, edit := range req.Edits {
		editCopy := edit

		// Guard: explicit field validation (prevents panic from nil pointer dereference)
		if reason := s.validator.Validate(&editCopy); reason != "" {
			resp.Rejected = append(resp.Rejected, model.IndexedReason{Index: i, Reason: reason})
			continue
		}

		result := s.validation.Validate(token, &editCopy)
		switch result.Outcome {
		case OutcomeRejected:
			resp.Rejected = append(resp.Rejected, model.IndexedReason{
				Index:  i,
				Reason: strings.Join(result.Reasons, ","),
			})
		case OutcomeFlagged:
			resp.Flagged = append(resp.Flagged, model.IndexedReason{
				Index:  i,
				Reason: strings.Join(result.Reasons, ","),
			})
			s.saveEdit(token, &editCopy, "FLAGGED", result.Reasons)
		case OutcomeAccepted:
			resp.Accepted++
			s.saveEdit(token, &editCopy, "ACCEPTED", nil)
		}
	}

	return resp
}

func (s *IngestService) saveEdit(token *model.Token, edit *model.EditDTO, status string, flags []string) {
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
		rec.DiffHunk = *edit.DiffHunk
	}
	if edit.Metadata != nil {
		rec.Metadata = *edit.Metadata
	}
	if len(flags) > 0 {
		rec.Flags = strings.Join(flags, ",")
	}
	s.editRepo.Save(rec)
}

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
