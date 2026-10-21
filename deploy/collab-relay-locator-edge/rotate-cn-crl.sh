#!/bin/sh
set -eu
LC_ALL=C
export LC_ALL

script_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
compose_file="$script_dir/compose.cn.yaml"

if [ "$#" -ne 2 ]; then
    echo "usage: $0 NEW_CRL.pem REVOKED_GLOBAL_LOCATOR_EDGE_CERT.pem" >&2
    exit 2
fi
candidate=$1
revoked_cert=$2
: "${OPENPENCIL_LOCATOR_EDGE_CLIENT_CRL:?set the active absolute locator-edge CRL path}"
: "${OPENPENCIL_LOCATOR_EDGE_CLIENT_CA:?set the absolute locator-edge client CA path}"
: "${OPENPENCIL_LOCATOR_EDGE_EXPECTED_CN_FEDERATION_UPSTREAM:?set fixed CN federation IPv4:port}"
: "${OPENPENCIL_LOCATOR_EDGE_EXPECTED_CN_INNER_HTTPS_UPSTREAM:?set fixed CN inner HTTPS IPv4:port}"
: "${OPENPENCIL_LOCATOR_EDGE_EXPECTED_CN_FEDERATION_NAME:?set outer TLS DNS name}"
: "${OPENPENCIL_LOCATOR_EDGE_EXPECTED_INNER_HOST:?set common client hostname}"
active=$OPENPENCIL_LOCATOR_EDGE_CLIENT_CRL
client_ca=$OPENPENCIL_LOCATOR_EDGE_CLIENT_CA
backup="${active}.previous"

if [ "$(id -u)" -ne 0 ]; then
    echo "CRL activation requires root" >&2
    exit 1
fi
for command_name in docker openssl date stat mktemp cp mv chmod chown sed tr grep sort comm; do
    command -v "$command_name" >/dev/null 2>&1 || {
        echo "required command is unavailable: $command_name" >&2
        exit 1
    }
done
for path in "$candidate" "$revoked_cert" "$active" "$client_ca"; do
    case "$path" in
        /*) ;;
        *)
            echo "CRL rotation paths must be absolute: $path" >&2
            exit 2
            ;;
    esac
    if [ ! -f "$path" ] || [ -L "$path" ]; then
        echo "CRL rotation paths must be regular non-symlink files: $path" >&2
        exit 2
    fi
done
[ "$candidate" != "$active" ] || {
    echo "candidate and active CRL paths must differ" >&2
    exit 2
}
if [ -e "$backup" ] || [ -L "$backup" ]; then
    echo "remove the verified previous CRL backup before rotating: $backup" >&2
    exit 2
fi

read_stat() {
    linux_format=$1
    bsd_format=$2
    path=$3
    stat -c "$linux_format" "$path" 2>/dev/null ||
        stat -f "$bsd_format" "$path"
}

active_dir=$(CDPATH= cd -- "$(dirname -- "$active")" && pwd)
ca_dir=$(CDPATH= cd -- "$(dirname -- "$client_ca")" && pwd)
active_dir_mode=$(read_stat '%a' '%Lp' "$active_dir")
active_mode=$(read_stat '%a' '%Lp' "$active")
ca_dir_mode=$(read_stat '%a' '%Lp' "$ca_dir")
ca_mode=$(read_stat '%a' '%Lp' "$client_ca")
active_dir_uid=$(read_stat '%u' '%u' "$active_dir")
ca_dir_uid=$(read_stat '%u' '%u' "$ca_dir")
active_uid=$(read_stat '%u' '%u' "$active")
active_gid=$(read_stat '%g' '%g' "$active")
ca_uid=$(read_stat '%u' '%u' "$client_ca")
ca_gid=$(read_stat '%g' '%g' "$client_ca")
case "$active_dir_mode:$active_mode:$ca_dir_mode:$ca_mode" in
    *[!0-7:]*)
        echo "CRL/CA paths have unsupported filesystem modes" >&2
        exit 1
        ;;
esac
if [ "$active_dir_uid" -ne 0 ] || [ "$ca_dir_uid" -ne 0 ] ||
    [ "$active_uid" -ne 0 ] || [ "$ca_uid" -ne 0 ] ||
    [ "$active_gid" -ne 101 ] || [ "$ca_gid" -ne 101 ] ||
    [ "$active_mode" != 440 ] || [ "$ca_mode" != 440 ] ||
    [ $((0$active_dir_mode & 0022)) -ne 0 ] ||
    [ $((0$ca_dir_mode & 0022)) -ne 0 ] ||
    [ $((0$active_mode & 0222)) -ne 0 ] ||
    [ $((0$ca_mode & 0222)) -ne 0 ]
then
    echo "CRL/CA files must be root:101 mode 0440 in root-owned secure directories" >&2
    exit 1
fi

OPENPENCIL_LOCATOR_EDGE_VALIDATION_MODE=production \
    "$script_dir/validate.sh" >/dev/null
docker compose -f "$compose_file" config -q

umask 077
staged=$(mktemp "$active_dir/.openpencil-locator-crl.new.XXXXXX")
staged_cert=$(mktemp "$active_dir/.openpencil-locator-cert.revoked.XXXXXX")
active_serials=
candidate_serials=
cleanup() {
    rm -f "$staged" "$staged_cert"
    [ -z "$active_serials" ] || rm -f "$active_serials"
    [ -z "$candidate_serials" ] || rm -f "$candidate_serials"
}
trap cleanup EXIT HUP INT TERM
cp "$candidate" "$staged"
cp "$revoked_cert" "$staged_cert"

verify_crl() {
    openssl crl -in "$1" -noout -verify \
        -CAfile "$client_ca" -no-CApath -no-CAstore >/dev/null
}
verify_crl "$active"
verify_crl "$staged"
openssl verify -no_check_time -CAfile "$client_ca" \
    -no-CApath -no-CAstore "$staged_cert" >/dev/null

normalize_hex() {
    value=$(printf '%s\n' "$1" |
        tr 'A-F' 'a-f' |
        sed 's/^0x//; s/://g; s/[[:space:]]//g; s/^0*//')
    [ -n "$value" ] || value=0
    case "$value" in
        *[!0-9a-f]*) return 1 ;;
    esac
    printf '%s\n' "$value"
}
read_crl_number() {
    raw=$(openssl crl -in "$1" -noout -crlnumber |
        sed -n 's/^crlNumber=//p')
    [ -n "$raw" ] || {
        echo "CRLNumber is required: $1" >&2
        return 1
    }
    normalize_hex "$raw"
}
active_number=$(read_crl_number "$active")
candidate_number=$(read_crl_number "$staged")
if [ "${#candidate_number}" -lt "${#active_number}" ] ||
    { [ "${#candidate_number}" -eq "${#active_number}" ] &&
        ! [ "$candidate_number" \> "$active_number" ]; }
then
    echo "candidate CRLNumber must be strictly greater than active CRLNumber" >&2
    exit 1
fi

parse_epoch() {
    date -u -d "$1" '+%s' 2>/dev/null ||
        date -j -u -f '%Y-%m-%d %H:%M:%SZ' "$1" '+%s'
}
read_crl_time() {
    value=$(openssl crl -in "$2" -noout "$1" -dateopt iso_8601 |
        sed -n 's/^[^=]*=//p')
    [ -n "$value" ] && [ "$value" != NONE ] || return 1
    parse_epoch "$value"
}
now=$(date -u '+%s')
last_update=$(read_crl_time -lastupdate "$staged")
next_update=$(read_crl_time -nextupdate "$staged")
if [ "$last_update" -gt $((now + 300)) ]; then
    echo "candidate CRL lastUpdate is in the future" >&2
    exit 1
fi
if [ "$next_update" -le $((now + 300)) ]; then
    echo "candidate CRL expires too soon or is expired" >&2
    exit 1
fi
if openssl crl -in "$staged" -noout -text |
    grep -F 'X509v3 Delta CRL Indicator' >/dev/null
then
    echo "delta CRLs are forbidden; publish a complete CRL" >&2
    exit 1
fi

active_serials=$(mktemp "$active_dir/.openpencil-locator-crl.active.XXXXXX")
candidate_serials=$(mktemp "$active_dir/.openpencil-locator-crl.candidate.XXXXXX")
extract_serials() {
    openssl crl -in "$1" -noout -text |
        sed -n 's/^[[:space:]]*Serial Number:[[:space:]]*//p' |
        tr 'A-F' 'a-f' |
        sed 's/://g; s/[[:space:]]//g; s/^0*//; s/^$/0/' |
        sort -u
}
extract_serials "$active" >"$active_serials"
extract_serials "$staged" >"$candidate_serials"
if comm -23 "$active_serials" "$candidate_serials" | grep -q .; then
    echo "candidate CRL drops an existing revoked certificate serial" >&2
    exit 1
fi
revoked_serial=$(openssl x509 -in "$staged_cert" -noout -serial |
    sed -n 's/^serial=//p')
revoked_serial=$(normalize_hex "$revoked_serial")
grep -Fx "$revoked_serial" "$candidate_serials" >/dev/null || {
    echo "candidate CRL does not revoke the supplied Global locator-edge certificate" >&2
    exit 1
}

rm -f "$active_serials" "$candidate_serials" "$staged_cert"
active_serials=
candidate_serials=
chmod "$active_mode" "$staged"
chown "$active_uid:$active_gid" "$staged"

# Revalidate the exact staged inode after its final ownership/mode changes.
verify_crl "$staged"
[ ! -L "$staged" ] && [ -f "$staged" ] || exit 1
[ "$(read_stat '%u' '%u' "$staged")" -eq "$active_uid" ]
[ "$(read_stat '%g' '%g' "$staged")" -eq "$active_gid" ]
[ "$(read_stat '%a' '%Lp' "$staged")" = "$active_mode" ]

trap - EXIT HUP INT TERM
published=0
rollback() {
    status=$?
    trap - EXIT HUP INT TERM
    rm -f "$staged"
    if [ "$published" -eq 1 ] && [ -f "$backup" ]; then
        mv -f "$backup" "$active"
        docker compose -f "$compose_file" up -d --no-deps --force-recreate \
            locator-cn-federation >/dev/null 2>&1 || true
    else
        rm -f "$backup"
    fi
    exit "$status"
}
trap rollback EXIT
trap 'exit 130' HUP INT TERM

cp -p "$active" "$backup"
published=1
mv -f "$staged" "$active"
docker compose -f "$compose_file" up -d --no-deps --force-recreate \
    locator-cn-federation
docker compose -f "$compose_file" exec -T locator-cn-federation nginx -t

published=0
trap - EXIT HUP INT TERM
echo "locator CN federation CRL published atomically and container recreated"
echo "Previous CRL retained at: $backup"
echo "Probe revoked and unrevoked locator-edge certificates before removing it."
