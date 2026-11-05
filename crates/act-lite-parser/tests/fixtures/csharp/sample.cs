using System;
using System.Collections.Generic;
using System.Threading.Tasks;

namespace Sample.Application;

/// <summary>
/// Represents a user in the system.
/// </summary>
public class User
{
    private readonly int _id;
    private string _name;

    public int Id => _id;
    public string Name { get => _name; set => _name = value ?? throw new ArgumentNullException(); }
    public DateTime CreatedAt { get; init; }

    public event EventHandler<string>? NameChanged;

    public User(int id, string name)
    {
        _id = id;
        _name = name;
    }

    public void UpdateName(string newName)
    {
        _name = newName;
        NameChanged?.Invoke(this, newName);
    }
}

public interface IRepository<T> where T : class
{
    Task<T?> GetByIdAsync(int id);
    Task<IEnumerable<T>> GetAllAsync();
    Task SaveAsync(T entity);
}

public enum Status
{
    Active,
    Inactive,
    Pending
}

public record Person(string FirstName, string LastName);

public struct Point
{
    public int X { get; init; }
    public int Y { get; init; }
}

public delegate void UserCreatedHandler(User user);

public static class Extensions
{
    public static bool IsEmpty(this string? s) => string.IsNullOrEmpty(s);
}
