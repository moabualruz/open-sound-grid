abort "CI must use a GitHub-hosted runner" unless ENV.fetch("RUNNER_ENVIRONMENT") == "github-hosted"

puts "CI runner trust boundary verified"
