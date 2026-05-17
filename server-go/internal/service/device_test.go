package service_test

import (
	"testing"

	"github.com/aitrack/server/internal/service"
	"github.com/aitrack/server/internal/testkit"
)

func TestHeartbeat_RecordAndList(t *testing.T) {
	database := openTestDB(t)
	deviceRepo := service.NewDeviceRepository(database)
	editRepo := service.NewEditRecordRepository(database)
	heartbeatSvc := service.NewHeartbeatService(deviceRepo)
	statsSvc := service.NewStatsService(editRepo, deviceRepo)

	token := testkit.BuildTokenWithSig()
	req := testkit.BuildHeartbeatRequest()

	if err := heartbeatSvc.RecordHeartbeat(token, req); err != nil {
		t.Fatal(err)
	}

	devices, err := statsSvc.GetDevices()
	if err != nil {
		t.Fatal(err)
	}
	if len(devices) != 1 {
		t.Fatalf("expected 1 device, got %d", len(devices))
	}
	d := devices[0]
	if d.DeviceID != req.DeviceID {
		t.Errorf("device_id mismatch: got %s, want %s", d.DeviceID, req.DeviceID)
	}
	if d.TokenKey != token.TokenKey {
		t.Errorf("token_key mismatch: got %s, want %s", d.TokenKey, token.TokenKey)
	}
	if d.LastHeartbeat == nil {
		t.Error("last_heartbeat should be set")
	}
}

func TestHeartbeat_Upsert(t *testing.T) {
	database := openTestDB(t)
	deviceRepo := service.NewDeviceRepository(database)
	heartbeatSvc := service.NewHeartbeatService(deviceRepo)

	token := testkit.BuildTokenWithSig()

	// First heartbeat
	req1 := testkit.BuildHeartbeatRequest(func(r *testkit.HeartbeatReq) {
		r.ClientVersion = "1.0.0"
	})
	heartbeatSvc.RecordHeartbeat(token, req1)

	// Second heartbeat — same device, new version
	req2 := testkit.BuildHeartbeatRequest(func(r *testkit.HeartbeatReq) {
		r.ClientVersion = "1.1.0"
	})
	heartbeatSvc.RecordHeartbeat(token, req2)

	devices, _ := deviceRepo.ListAll()
	if len(devices) != 1 {
		t.Fatalf("expected 1 device after upsert, got %d", len(devices))
	}
	if devices[0].ClientVersion != "1.1.0" {
		t.Errorf("expected updated version 1.1.0, got %s", devices[0].ClientVersion)
	}
}

func TestHeartbeat_NilHooks(t *testing.T) {
	database := openTestDB(t)
	deviceRepo := service.NewDeviceRepository(database)
	heartbeatSvc := service.NewHeartbeatService(deviceRepo)

	token := testkit.BuildTokenWithSig()
	req := testkit.BuildHeartbeatRequest(func(r *testkit.HeartbeatReq) {
		r.Hooks = nil
	})
	if err := heartbeatSvc.RecordHeartbeat(token, req); err != nil {
		t.Fatal(err)
	}
}

func TestStats_AggregateByToken(t *testing.T) {
	database := openTestDB(t)
	editRepo := service.NewEditRecordRepository(database)
	deviceRepo := service.NewDeviceRepository(database)
	statsSvc := service.NewStatsService(editRepo, deviceRepo)

	// No records yet — should return empty list without error
	rows, err := statsSvc.GetStats("token")
	if err != nil {
		t.Fatal(err)
	}
	if rows == nil {
		t.Error("should return non-nil slice")
	}
}

func TestStats_GetDevices_SilentFlag(t *testing.T) {
	database := openTestDB(t)
	deviceRepo := service.NewDeviceRepository(database)
	editRepo := service.NewEditRecordRepository(database)
	statsSvc := service.NewStatsService(editRepo, deviceRepo)
	heartbeatSvc := service.NewHeartbeatService(deviceRepo)

	token := testkit.BuildTokenWithSig()
	// Device with no heartbeat
	deviceRepo.Upsert(testkit.DeviceNoHeartbeat("dev-silent", token.TokenKey))

	// Device with recent heartbeat
	heartbeatSvc.RecordHeartbeat(token, testkit.BuildHeartbeatRequest(func(r *testkit.HeartbeatReq) {
		r.DeviceID = "dev-active"
	}))

	devices, err := statsSvc.GetDevices()
	if err != nil {
		t.Fatal(err)
	}
	if len(devices) < 2 {
		t.Fatalf("expected 2 devices, got %d", len(devices))
	}
	for _, d := range devices {
		if d.DeviceID == "dev-silent" && !d.Silent {
			t.Error("dev-silent should be marked silent")
		}
		if d.DeviceID == "dev-active" && d.Silent {
			t.Error("dev-active should not be silent")
		}
	}
}
