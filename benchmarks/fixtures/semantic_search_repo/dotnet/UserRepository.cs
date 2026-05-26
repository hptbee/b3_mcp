public class UserRepository
{
    public string FindUserByEmail(string email)
    {
        const string sql = "select id, email from users where email = @email";
        return sql;
    }
}
