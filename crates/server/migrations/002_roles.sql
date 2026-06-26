CREATE TABLE roles (
    id         UUID        PRIMARY KEY DEFAULT uuid_generate_v4(),
    tenant_id  UUID        NOT NULL,
    name       TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_roles_tenant_name UNIQUE (tenant_id, name)
);

CREATE TABLE user_roles (
    user_id    UUID        NOT NULL REFERENCES users(id)  ON DELETE CASCADE,
    role_id    UUID        NOT NULL REFERENCES roles(id)  ON DELETE CASCADE,
    granted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, role_id)
);
