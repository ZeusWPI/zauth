;; Example usage:
;; guix shell -mmanifest.scm -CFN -Sbin/cc=bin/gcc -ETERM --share=$HOME/.cargo -- sh -c 'LD_LIBRARY_PATH=$LIBRARY_PATH cargo build'

(use-modules (guix channels)
             (guix inferior)
             (guix profiles)
             (guix ui)
             (srfi srfi-11)
             (srfi srfi-26))

(define (pkgs channels specs)
  (let* ((inferior (inferior-for-channels channels))
         (lookup (cut lookup-inferior-packages inferior <> <>)))
    (map (lambda (spec)
           (let-values (((name version output)
                         (package-specification->name+version+output spec)))
             (list (car (lookup name version))
                   output)))
         specs)))

(packages->manifest
 (pkgs (list (channel
               (inherit %default-guix-channel)
               (commit "dd080e7fda2be54e2bcec3814473f90b326cb256")))
       (list "bash" "coreutils"
             "gcc-toolchain" "pkg-config" "postgresql" "openssl"
             "rust" "rust:cargo" "rust:rust-src" "rust:tools"
             "node"
             "python" "python-flask" "python-requests"
             "nss-certs" "man-db")))
