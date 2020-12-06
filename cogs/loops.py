from os import listdir, remove
from traceback import format_exception

import discord
from discord.ext import commands, tasks

from utils import basic

def setup(bot):
    bot.add_cog(loops(bot))

class loops(commands.Cog):
    def __init__(self, bot):
        self.bot = bot
        self.cache_cleanup.start()

    def cog_unload(self):
        self.cache_cleanup.cancel()

    @tasks.loop(seconds=60.0)
    async def cache_cleanup(self):
        try:
            cache_size = basic.get_size("cache")
            if cache_size >= 1073741824:
                print("Deleting 100 messages from cache!")
                cache_folder = listdir("cache")
                cache_folder.sort(reverse=False, key=lambda x: int(''.join(filter(str.isdigit, x))))

                for count, cached_message in enumerate(cache_folder):
                    remove(f"cache/{cached_message}")
                    self.bot.cache.remove(cached_message)

                    if count == 100: break

        except Exception as e:
            error = getattr(e, 'original', e)

            temp = f"```{''.join(format_exception(type(error), error, error.__traceback__))}```"
            if len(temp) >= 1900:
                with open("temp.txt", "w") as f:  f.write(temp)
                await self.bot.channels["errors"].send(file=discord.File("temp.txt"))
            else:
                await self.bot.channels["errors"].send(temp)

    @cache_cleanup.before_loop
    async def before_file_saving_loop(self):
        await self.bot.wait_until_ready()
