#!/usr/bin/env ruby
# frozen_string_literal: true

require 'psych'
require 'set'

class ValidationError < StandardError; end

class DuplicateKeyDetectingHandler < Psych::Handler
  MappingState = Struct.new(:keys, :expecting_key)

  def initialize(path)
    super()
    @path = path
    @stack = []
    @documents = 0
  end

  attr_reader :documents

  def start_document(*)
    @documents += 1
  end

  def start_mapping(*)
    parent = @stack.last
    if parent.is_a?(MappingState)
      if parent.expecting_key
        raise ValidationError, "Unsupported non-scalar mapping key in #{@path}"
      end
      parent.expecting_key = true
    end

    @stack << MappingState.new(Set.new, true)
  end

  def end_mapping
    @stack.pop
  end

  def start_sequence(*)
    parent = @stack.last
    if parent.is_a?(MappingState) && !parent.expecting_key
      parent.expecting_key = true
    end
    @stack << :sequence
  end

  def end_sequence
    @stack.pop
  end

  def scalar(value, *)
    parent = @stack.last
    return unless parent.is_a?(MappingState)

    if parent.expecting_key
      key = value.to_s
      raise ValidationError, "Duplicate key '#{key}' in #{@path}" if parent.keys.include?(key)

      parent.keys << key
      parent.expecting_key = false
    else
      parent.expecting_key = true
    end
  end

  def alias(*)
    parent = @stack.last
    if parent.is_a?(MappingState) && !parent.expecting_key
      parent.expecting_key = true
    end
  end
end

def validate_file!(path)
  content = File.read(path)
  if content.strip.empty?
    raise ValidationError, "Empty YAML stream: #{path}"
  end

  handler = DuplicateKeyDetectingHandler.new(path)
  parser = Psych::Parser.new(handler)
  parser.parse(content, path)

  raise ValidationError, "No YAML documents in #{path}" if handler.documents.zero?
end

begin
  if ARGV.empty?
    warn 'Usage: scripts/validate-yaml-stream.rb <yaml-file> [<yaml-file> ...]'
    exit 2
  end

  ARGV.each do |path|
    validate_file!(path)
    puts "YAML OK: #{path}"
  end
rescue ValidationError => e
  warn "YAML validation failed: #{e.message}"
  exit 1
rescue Errno::ENOENT => e
  warn "YAML validation failed: #{e.message}"
  exit 1
rescue Psych::SyntaxError => e
  warn "YAML validation failed: #{e.message}"
  exit 1
end
