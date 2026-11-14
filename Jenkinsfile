// Copyright (c) 2026 MirraNET, Niklas Linz. Licensed under the GNU LGPLv3.
//
// Jenkins build of FreeX Desktop on a Linux agent: checks and tests, the Linux app with its
// Debian package, the Windows app (cross-compiled) with its installer; builds of main go into
// the MirraNET APT repository. The macOS app is built on the Mac node "Spark" (last stage). For a package that installs on Debian 12+ and Ubuntu 22.04+,
// the agent itself should be Debian 12 or Ubuntu 22.04 (glibc, see packaging/deb/build-deb.sh).
//
// The agent needs, besides rustup and the Windows tools listed in packaging/windows/build.sh:
//
//   sudo apt install build-essential pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev \
//       libayatana-appindicator3-dev librsvg2-dev librsvg2-bin dpkg binutils \
//       gcc-mingw-w64-x86-64 nsis curl unzip
//
// Artifacts in dist/: the .deb, the setup .exe and the plain Windows app, and from Spark the
// .dmg and .zip of the Mac app. Spark should only take jobs that ask for it (node setting
// "Usage: Only build jobs with label expressions matching this node"), so `agent any` does not
// put the Linux stages on the Mac.
pipeline {
    agent any

    options {
        buildDiscarder(logRotator(numToKeepStr: '20', artifactNumToKeepStr: '10'))
        disableConcurrentBuilds()
        timeout(time: 120, unit: 'MINUTES')
    }

    environment {
        PATH = "${env.HOME}/.cargo/bin:${env.PATH}"
        CARGO_TERM_COLOR = 'never'
    }

    stages {
        stage('Check') {
            steps {
                // The workspace is kept between builds; packages of earlier versions must go.
                sh 'rm -rf dist'
                dir('src-tauri') {
                    sh 'cargo fmt --check'
                    sh 'cargo clippy --locked --all-targets -- -D warnings'
                }
            }
        }

        stage('Test') {
            steps {
                dir('src-tauri') {
                    sh 'cargo test --locked'
                }
            }
        }

        stage('Linux') {
            steps {
                dir('src-tauri') {
                    sh 'cargo build --release --locked --features custom-protocol'
                }
                sh '''
                    VERSION="$(sed -n 's/^version *= *"\\(.*\\)"/\\1/p' src-tauri/Cargo.toml | head -n 1)"
                    # Every build gets its own package version, newer than the one before, so apt
                    # sees it as an update even while Cargo.toml keeps the same version:
                    # 1.1.1+202611142030.gitabc1234 (the app itself keeps showing 1.1.1)
                    COMMIT="$(git rev-parse --short=7 HEAD 2>/dev/null || echo local)"
                    DEB_VERSION="$VERSION+$(date -u +%Y%m%d%H%M).git$COMMIT"
                    packaging/deb/build-deb.sh "$DEB_VERSION" src-tauri/target/release/freex-desktop dist
                '''
            }
        }

        stage('Windows') {
            steps {
                sh 'packaging/windows/build.sh'
            }
        }

        // Puts the .deb into the MirraNET APT repository (apt.mirranet.de) with
        // packaging/apt/publish.sh. Set up in Jenkins (Global properties): APT_REPO, the
        // repository directory; APT_HOST, user@machine of the repository if it is not this one
        // (reached by SSH key); APT_SUITE, default stable. Only builds of main are published.
        stage('Publish') {
            when {
                expression { env.APT_REPO?.trim() && (env.BRANCH_NAME == null || env.BRANCH_NAME == 'main') }
            }
            steps {
                script {
                    if (env.APT_HOST?.trim()) {
                        // Repository on another machine: copy the packages and the script there and
                        // run it with that machine's reprepro and signing key.
                        sh '''
                            set -eu
                            tmp=$(ssh -o BatchMode=yes "$APT_HOST" mktemp -d)
                            scp -o BatchMode=yes -q packaging/apt/publish.sh dist/*.deb "$APT_HOST:$tmp/"
                            ssh -o BatchMode=yes "$APT_HOST" bash -s -- "$tmp" "$APT_REPO" "${APT_SUITE:-stable}" <<'EOF'
rc=0
bash "$1/publish.sh" "$2" "$3" "$1"/*.deb || rc=$?
rm -rf "$1"
exit $rc
EOF
                        '''
                    } else {
                        sh 'packaging/apt/publish.sh "$APT_REPO" "${APT_SUITE:-stable}" dist/*.deb'
                    }
                }
            }
        }

        // The macOS app on the Mac node "Spark" (own workspace there). It needs Xcode or the
        // Command Line Tools and rustup in the home of the Jenkins user on Spark. Comes last, so
        // the Linux package is published even while the Mac is offline.
        stage('macOS') {
            agent { label 'Spark' }
            options {
                timeout(time: 60, unit: 'MINUTES')
            }
            steps {
                sh '''
                    # PATH of the pipeline comes from the Linux agent; the Mac has its own.
                    export PATH="$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"
                    rm -rf dist
                    (cd src-tauri && cargo clippy --locked --all-targets -- -D warnings && cargo test --locked)
                    packaging/macos/build.sh "$BUILD_NUMBER"
                '''
            }
            post {
                success {
                    archiveArtifacts artifacts: 'dist/*-macos.dmg, dist/*-macos.zip', fingerprint: true
                }
            }
        }
    }

    post {
        success {
            archiveArtifacts artifacts: 'dist/*.deb, dist/freex-desktop-*-setup.exe, dist/freex-desktop.exe, dist/WebView2Loader.dll', fingerprint: true
        }
    }
}
