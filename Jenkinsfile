// Copyright (c) 2026 MirraNET, Niklas Linz. Licensed under the GNU LGPLv3.
//
// Jenkins build of FreeX Desktop on a Linux agent: checks and tests, the Linux app with its
<<<<<<< Updated upstream
// Debian package, the Windows app (cross-compiled) with its installer. The macOS app is built on
// the Mac node "Spark" (last stage). Releases go into the MirraNET APT repository from GitHub
// Actions (.github/workflows/build.yml, job apt), not from here. For a package that installs on
// Debian 12+ and Ubuntu 22.04+, the agent itself should be Debian 12 or Ubuntu 22.04 (glibc, see packaging/deb/build-deb.sh).
=======
// Debian package, the Windows app (cross-compiled) with its installer. The macOS app is built on the Mac node "Spark" (last stage). For a package that installs on Debian 12+ and Ubuntu 22.04+,
// the agent itself should be Debian 12 or Ubuntu 22.04 (glibc, see packaging/deb/build-deb.sh).
>>>>>>> Stashed changes
//
// The agent needs, besides rustup and the Windows tools listed in packaging/windows/build.sh:
//
//   sudo apt install build-essential pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev \
//       libayatana-appindicator3-dev librsvg2-dev librsvg2-bin dpkg binutils \
//       gcc-mingw-w64-x86-64 nsis curl unzip git cmake clang
//
// (cmake 3.29+ and clang build makemsix once, for the Store package; see packaging/msix/build.sh.
// The package identity from Partner Center comes from the Global properties MSIX_IDENTITY,
// MSIX_PUBLISHER and MSIX_PUBLISHER_NAME.)
//
// Artifacts in dist/: the .deb, the setup .exe, the .msix for the Microsoft Store and the plain
// Windows app, and from Spark the
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
                // The TV mode's interface (ui/tv) from FreeX-TV-Apps, branch TV_APPS_REF (main).
                sh 'packaging/tv-ui.sh'
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
                    # Every build gets its own package version, newer than the one before, so it
                    # installs as an update even while Cargo.toml keeps the same version:
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
                // The same app as an MSIX package for the Microsoft Store (unsigned; to upload in
                // Partner Center, the Store signs it).
                sh 'packaging/msix/build.sh'
            }
        }

        // The macOS app on the Mac node "Spark" (own workspace there). It needs Xcode or the
        // Command Line Tools and rustup in the home of the Jenkins user on Spark. Comes last, so
<<<<<<< Updated upstream
        // the other builds are archived even while the Mac is offline.
=======
        // the other artifacts are built even while the Mac is offline.
>>>>>>> Stashed changes
        stage('macOS') {
            agent { label 'Spark' }
            options {
                timeout(time: 60, unit: 'MINUTES')
            }
            // The PATH of the pipeline is the Linux agent's (without it, Jenkins does not even
            // find sh on the Mac); this stage uses the Mac's usual one. rustup's comes in the step.
            environment {
                PATH = '/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin'
            }
            steps {
                sh '''
                    export PATH="$HOME/.cargo/bin:$PATH"
                    # rustup may have been installed with the minimal profile; adds what is missing.
                    rustup component add clippy rustfmt
                    rm -rf dist
                    packaging/tv-ui.sh
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
            archiveArtifacts artifacts: 'dist/*.deb, dist/freex-desktop-*-setup.exe, dist/freex-desktop-*-x64.msix, dist/freex-desktop.exe, dist/WebView2Loader.dll', fingerprint: true
        }
    }
}
