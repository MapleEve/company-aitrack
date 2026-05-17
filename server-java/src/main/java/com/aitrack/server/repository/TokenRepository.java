package com.aitrack.server.repository;

import com.aitrack.server.entity.TokenEntity;
import org.springframework.data.jpa.repository.JpaRepository;
import org.springframework.stereotype.Repository;

import java.util.Optional;

@Repository
public interface TokenRepository extends JpaRepository<TokenEntity, Long> {
    Optional<TokenEntity> findByTokenHashAndActiveTrue(String tokenHash);
}
