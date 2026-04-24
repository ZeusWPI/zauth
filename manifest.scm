;; Example usage:
;; guix shell -mmanifest.scm -CFN -Sbin/cc=bin/gcc -ETERM --share=$HOME/.cargo -- sh -c 'LD_LIBRARY_PATH=$LIBRARY_PATH cargo build'

(use-modules (guix channels)
             (guix inferior)
             (guix profiles)
             (guix ui)
             (srfi srfi-11))

(define channels
  (list
   (channel
     (name 'guix)
     (url "https://git.guix.gnu.org/guix.git")
     (branch "master")
     (commit "dd080e7fda2be54e2bcec3814473f90b326cb256")
     (introduction
      (make-channel-introduction
       "9edb3f66fd807b096b48283debdcddccfea34bad"
       (openpgp-fingerprint
        "BBB0 2DDF 2CEA F6A8 0D1D  E643 A2A0 6DF2 A33A 54FA"))))))

(define inferior
  (inferior-for-channels channels))

(define (pkg spec)
  (let-values (((name version output)
                (package-specification->name+version+output spec)))
    (list (car (lookup-inferior-packages inferior name version))
          output)))
(define (pkgs . args)
  (map pkg args))

(packages->manifest
 (pkgs "bash" "coreutils"
       "gcc-toolchain" "pkg-config" "postgresql" "openssl"
       "rust" "rust:cargo" "rust:rust-src" "rust:tools"
       "node"
       "python" "python-flask" "python-requests"
       "nss-certs" "man-db"))
