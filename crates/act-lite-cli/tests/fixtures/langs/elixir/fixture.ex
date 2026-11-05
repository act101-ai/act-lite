defmodule Greeter do
  def hello(name) do
    "hello " <> name
  end

  def twice(name) do
    hello(name) <> hello(name)
  end
end
