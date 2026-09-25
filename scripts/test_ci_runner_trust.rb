require "yaml"

workflow = YAML.load_file(".github/workflows/ci.yml")
guard = workflow.fetch("jobs").fetch("test").fetch("if")
expected_guard = "github.event_name != 'pull_request' || github.event.pull_request.head.repo.full_name == github.repository"
abort "unexpected runner trust guard: #{guard.inspect}" unless guard == expected_guard

fixtures = [
  { name: "same-repository PR", event: "pull_request", repository: "owner/repo", head_repository: "owner/repo", allowed: true },
  { name: "fork PR", event: "pull_request", repository: "owner/repo", head_repository: "contributor/repo", allowed: false },
  { name: "manual dispatch", event: "workflow_dispatch", repository: "owner/repo", head_repository: nil, allowed: true },
]

fixtures.each do |fixture|
  allowed = fixture[:event] != "pull_request" || fixture[:head_repository] == fixture[:repository]
  abort "#{fixture[:name]}: expected #{fixture[:allowed]}, got #{allowed}" unless allowed == fixture[:allowed]
end

puts "runner trust event fixtures passed (#{fixtures.length})"
