--
-- PostgreSQL database dump
--

-- Dumped from database version 16.3
-- Dumped by pg_dump version 17.4 (Ubuntu 17.4-1.pgdg22.04+2)

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET transaction_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: heroku_ext; Type: SCHEMA; Schema: -; Owner: -
--

CREATE SCHEMA heroku_ext;


--
-- Name: dblink; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS dblink WITH SCHEMA public;


--
-- Name: EXTENSION dblink; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION dblink IS 'connect to other PostgreSQL databases from within a database';


--
-- Name: ltree; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS ltree WITH SCHEMA public;


--
-- Name: EXTENSION ltree; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION ltree IS 'data type for hierarchical tree-like structures';


--
-- Name: pg_stat_statements; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS pg_stat_statements WITH SCHEMA public;


--
-- Name: EXTENSION pg_stat_statements; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION pg_stat_statements IS 'track execution statistics of all SQL statements executed';


--
-- Name: pg_trgm; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;


--
-- Name: EXTENSION pg_trgm; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION pg_trgm IS 'text similarity measurement and index searching based on trigrams';



--
-- Name: pgcrypto; Type: EXTENSION; Schema: -; Owner: -
--

CREATE EXTENSION IF NOT EXISTS pgcrypto WITH SCHEMA public;


--
-- Name: EXTENSION pgcrypto; Type: COMMENT; Schema: -; Owner: -
--

COMMENT ON EXTENSION pgcrypto IS 'cryptographic functions';


--
-- Name: semver_triple; Type: TYPE; Schema: public; Owner: -
--

CREATE TYPE public.semver_triple AS (
	major numeric,
	minor numeric,
	teeny numeric
);


--
-- Name: canon_crate_name(text); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.canon_crate_name(text) RETURNS text
    LANGUAGE sql
    AS $_$
                    SELECT replace(lower($1), '-', '_')
                $_$;




--
-- Name: random_string(integer); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.random_string(integer) RETURNS text
    LANGUAGE sql
    AS $_$
  SELECT (array_to_string(array(
    SELECT substr(
      'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789',
      floor(random() * 62)::int4 + 1,
      1
    ) FROM generate_series(1, $1)
  ), ''))
$_$;




--
-- Name: semver_ord(character varying); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.semver_ord(num character varying) RETURNS jsonb
    LANGUAGE plpgsql IMMUTABLE
    AS $_$
declare
    -- We need to ensure that the prerelease array has the same length for all
    -- versions since shorter arrays have lower precedence in JSONB. We store
    -- the first 10 parts of the prerelease string as pairs of booleans and
    -- numbers or text values, and then a final text item for the remaining
    -- parts.
    max_prerelease_parts constant int := 10;

    -- We ignore the "build metadata" part of the semver string, since it has
    -- no impact on the version ordering.
    match_result text[] := regexp_match(num, '^(\d+).(\d+).(\d+)(?:-([0-9A-Za-z\-.]+))?');

    prerelease jsonb;
    prerelease_parts text[];
    prerelease_part text;
    i int := 0;
begin
    if match_result is null then
        return null;
    end if;

    if match_result[4] is null then
        -- A JSONB object has higher precedence than an array, and versions with
        -- prerelease specifiers should have lower precedence than those without.
        prerelease := json_build_object();
    else
        prerelease := to_jsonb(array_fill(NULL::bool, ARRAY[max_prerelease_parts * 2 + 1]));

        -- Split prerelease string by `.` and "append" items to
        -- the `prerelease` array.
        prerelease_parts := string_to_array(match_result[4], '.');

        foreach prerelease_part in array prerelease_parts[1:max_prerelease_parts + 1]
        loop
            -- Parse parts as numbers if they consist of only digits.
            if regexp_like(prerelease_part, '^\d+$') then
                -- In JSONB a number has higher precedence than a string but in
                -- semver it is the other way around, so we use true/false to
                -- work around this.
                prerelease := jsonb_set(prerelease, array[i::text], to_jsonb(false));
                prerelease := jsonb_set(prerelease, array[(i + 1)::text], to_jsonb(prerelease_part::numeric));
            else
                prerelease := jsonb_set(prerelease, array[i::text], to_jsonb(true));
                prerelease := jsonb_set(prerelease, array[(i + 1)::text], to_jsonb(prerelease_part));
            end if;

            -- Exit the loop if we have reached the maximum number of parts.
            i := i + 2;
            exit when i >= max_prerelease_parts * 2;
        end loop;

        prerelease := jsonb_set(prerelease, array[(max_prerelease_parts * 2)::text], to_jsonb(array_to_string(prerelease_parts[max_prerelease_parts + 1:], '.')));
    end if;

    -- Return an array with the major, minor, patch, and prerelease parts.
    return json_build_array(
        match_result[1]::numeric,
        match_result[2]::numeric,
        match_result[3]::numeric,
        prerelease
    );
end;
$_$;


--
-- Name: FUNCTION semver_ord(num character varying); Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON FUNCTION public.semver_ord(num character varying) IS 'Converts a semver string into a JSONB array for version comparison purposes. The array has the following format: [major, minor, patch, prerelease] and when used for sorting follow the precedence rules defined in the semver specification (https://semver.org/#spec-item-11).';


--
-- Name: set_category_path_to_slug(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.set_category_path_to_slug() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
 NEW.path = text2ltree('root.' || trim(replace(replace(NEW.slug, '-', '_'), '::', '.')));
 RETURN NEW;
END;
    $$;


--
-- Name: set_semver_ord(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.set_semver_ord() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
begin
    new.semver_ord := semver_ord(new.num);
    return new;
end
$$;


--
-- Name: set_updated_at(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.set_updated_at() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF (
        NEW IS DISTINCT FROM OLD AND
        NEW.updated_at IS NOT DISTINCT FROM OLD.updated_at
    ) THEN
        NEW.updated_at = CURRENT_TIMESTAMP;
    END IF;
    RETURN NEW;
END
$$;


--
-- Name: touch_crate(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.touch_crate() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
                BEGIN
                    IF TG_OP = 'DELETE' THEN
                        UPDATE crates SET updated_at = CURRENT_TIMESTAMP WHERE
                            id = OLD.crate_id;
                        RETURN OLD;
                    ELSE
                        UPDATE crates SET updated_at = CURRENT_TIMESTAMP WHERE
                            id = NEW.crate_id;
                        RETURN NEW;
                    END IF;
                END
                $$;


--
-- Name: touch_crate_on_version_modified(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.touch_crate_on_version_modified() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
  IF (
    TG_OP = 'INSERT' OR
    NEW.updated_at IS DISTINCT FROM OLD.updated_at
  ) THEN
    UPDATE crates SET updated_at = CURRENT_TIMESTAMP WHERE
      crates.id = NEW.crate_id;
  END IF;
  RETURN NEW;
END;
$$;




--
-- Name: update_categories_crates_cnt(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.update_categories_crates_cnt() RETURNS trigger
    LANGUAGE plpgsql
    AS $$ BEGIN IF (TG_OP = 'INSERT') THEN UPDATE categories SET crates_cnt = crates_cnt + 1 WHERE id = NEW.category_id; return NEW; ELSIF (TG_OP = 'DELETE') THEN UPDATE categories SET crates_cnt = crates_cnt - 1 WHERE id = OLD.category_id; return OLD; END IF; END $$;



--
-- Name: update_num_versions_from_versions(); Type: FUNCTION; Schema: public; Owner: -
--

CREATE FUNCTION public.update_num_versions_from_versions() RETURNS trigger
    LANGUAGE plpgsql
    AS $$
BEGIN
    IF (TG_OP = 'INSERT') THEN
        INSERT INTO default_versions (crate_id, version_id, num_versions)
        VALUES (NEW.crate_id, NEW.id, 1)
        ON CONFLICT (crate_id) DO UPDATE
        SET num_versions = default_versions.num_versions + 1;
        RETURN NEW;
    ELSIF (TG_OP = 'DELETE') THEN
        UPDATE default_versions
        SET num_versions = num_versions - 1
        WHERE crate_id = OLD.crate_id;
        RETURN OLD;
    END IF;
END
$$;


SET default_tablespace = '';

SET default_table_access_method = heap;



--
-- Name: crate_owners; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.crate_owners (
    crate_id integer NOT NULL,
    owner_id integer NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    created_by integer,
    deleted boolean DEFAULT false NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    owner_kind integer NOT NULL,
    email_notifications boolean DEFAULT true NOT NULL
);


--
-- Name: COLUMN crate_owners.owner_id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.crate_owners.owner_id IS 'This refers either to the `crate_users.id` or `teams.id` column, depending on the value of the `owner_kind` column';


--
-- Name: COLUMN crate_owners.owner_kind; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.crate_owners.owner_kind IS '`owner_kind = 0` refers to `crate_users`, `owner_kind = 1` refers to `teams`.';


--
-- Name: crates; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.crates (
    id integer NOT NULL,
    name character varying NOT NULL,
    updated_at timestamp with time zone DEFAULT now() NOT NULL,
    created_at timestamp with time zone DEFAULT now() NOT NULL,
    description character varying,
    homepage character varying,
    documentation character varying,
    readme character varying,
    repository character varying,
    max_upload_size integer,
    max_features smallint
);




--
-- Name: crate_users; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.crate_users (
    id integer NOT NULL,
    gh_access_token character varying NOT NULL,
    gh_login character varying NOT NULL,
    name character varying,
    gh_avatar character varying,
    gh_id integer NOT NULL,
    account_lock_reason character varying,
    account_lock_until timestamp with time zone,
    is_admin boolean DEFAULT false NOT NULL,
    publish_notifications boolean DEFAULT true NOT NULL
);


--
-- Name: COLUMN crate_users.publish_notifications; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.crate_users.publish_notifications IS 'Whether or not the user wants to receive notifications when a package they own is published';


--
-- Name: crate_downloads; Type: TABLE; Schema: public; Owner: -
--

CREATE TABLE public.crate_downloads (
    crate_id integer NOT NULL,
    downloads bigint DEFAULT 0 NOT NULL
);


--
-- Name: TABLE crate_downloads; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON TABLE public.crate_downloads IS 'Number of downloads per crate. This was extracted from the `crates` table for performance reasons.';


--
-- Name: COLUMN crate_downloads.crate_id; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.crate_downloads.crate_id IS 'Reference to the crate that this row belongs to.';


--
-- Name: COLUMN crate_downloads.downloads; Type: COMMENT; Schema: public; Owner: -
--

COMMENT ON COLUMN public.crate_downloads.downloads IS 'The total number of downloads for this crate.';



--
-- Name: users_id_seq; Type: SEQUENCE; Schema: public; Owner: -
--

CREATE SEQUENCE public.users_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;


--
-- Name: users_id_seq; Type: SEQUENCE OWNED BY; Schema: public; Owner: -
--

ALTER SEQUENCE public.users_id_seq OWNED BY public.crate_users.id;




--
-- Name: crate_users id; Type: DEFAULT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.crate_users ALTER COLUMN id SET DEFAULT nextval('public.users_id_seq'::regclass);




--
-- Name: crate_owners crate_owners_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.crate_owners
    ADD CONSTRAINT crate_owners_pkey PRIMARY KEY (crate_id, owner_id, owner_kind);




--
-- Name: crates packages_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.crates
    ADD CONSTRAINT packages_pkey PRIMARY KEY (id);



--
-- Name: crate_users users_pkey; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.crate_users
    ADD CONSTRAINT users_pkey PRIMARY KEY (id);


--
-- Name: crate_downloads crate_downloads_pk; Type: CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.crate_downloads
    ADD CONSTRAINT crate_downloads_pk PRIMARY KEY (crate_id);





--
-- Name: crate_owners_not_deleted; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX crate_owners_not_deleted ON public.crate_owners USING btree (crate_id, owner_id, owner_kind) WHERE (NOT deleted);


--
-- Name: crate_downloads_downloads_crate_id_index; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX crate_downloads_downloads_crate_id_index ON public.crate_downloads USING btree (downloads DESC, crate_id DESC);



--
-- Name: index_crate_created_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX index_crate_created_at ON public.crates USING btree (created_at);


--
-- Name: index_crate_updated_at; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX index_crate_updated_at ON public.crates USING btree (updated_at);





--
-- Name: index_crates_name; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX index_crates_name ON public.crates USING btree (public.canon_crate_name((name)::text));


--
-- Name: index_crates_name_ordering; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX index_crates_name_ordering ON public.crates USING btree (name);


--
-- Name: index_crates_name_tgrm; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX index_crates_name_tgrm ON public.crates USING gin (public.canon_crate_name((name)::text) public.gin_trgm_ops);



--
-- Name: lower_gh_login; Type: INDEX; Schema: public; Owner: -
--

CREATE INDEX lower_gh_login ON public.crate_users USING btree (lower((gh_login)::text));


--
-- Name: users_gh_id; Type: INDEX; Schema: public; Owner: -
--

CREATE UNIQUE INDEX users_gh_id ON public.crate_users USING btree (gh_id) WHERE (gh_id > 0);


--
-- Name: crate_owners trigger_crate_owners_set_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER trigger_crate_owners_set_updated_at BEFORE UPDATE ON public.crate_owners FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();


--
-- Name: crates trigger_crates_set_updated_at; Type: TRIGGER; Schema: public; Owner: -
--

CREATE TRIGGER trigger_crates_set_updated_at BEFORE UPDATE ON public.crates FOR EACH ROW EXECUTE FUNCTION public.set_updated_at();



--
-- Name: crate_owners fk_crate_owners_crate_id; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.crate_owners
    ADD CONSTRAINT fk_crate_owners_crate_id FOREIGN KEY (crate_id) REFERENCES public.crates(id) ON DELETE CASCADE;


--
-- Name: crate_owners fk_crate_owners_created_by; Type: FK CONSTRAINT; Schema: public; Owner: -
--

ALTER TABLE ONLY public.crate_owners
    ADD CONSTRAINT fk_crate_owners_created_by FOREIGN KEY (created_by) REFERENCES public.crate_users(id);




--
-- PostgreSQL database dump complete
--

